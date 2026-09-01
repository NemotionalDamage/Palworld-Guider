#include "pal_transport.h"

#include <windows.h>
#include <winhttp.h>

#include <array>
#include <atomic>
#include <condition_variable>
#include <cstring>
#include <deque>
#include <mutex>
#include <string>
#include <thread>

namespace {

constexpr size_t kDefaultFrameLimit = 65'536;
constexpr size_t kDefaultQueueLimit = 64;
constexpr size_t kReceiveChunkSize = 16'384;

struct Configuration {
    uint16_t port{};
    std::string bearer_token;
    size_t max_frame_bytes{kDefaultFrameLimit};
    size_t queue_limit{kDefaultQueueLimit};
};

struct TransportState {
    mutable std::mutex mutex;
    std::condition_variable send_ready;
    std::condition_variable receive_ready;
    Configuration configuration;
    std::deque<std::string> send_queue;
    std::deque<std::string> receive_queue;
    std::thread connector;
    std::thread sender;
    std::thread receiver;
    std::atomic<HINTERNET> websocket{};
    std::atomic<bool> stopping{false};
    std::atomic<bool> overflowed{false};
    std::atomic<GuiderTransportState> state{GuiderTransportState::Idle};
    std::atomic<DWORD> last_error{0};
    bool workers_started{false};
};

TransportState& transport() {
    static TransportState value;
    return value;
}

void set_state(GuiderTransportState next, DWORD error = 0) {
    transport().state.store(next);
    transport().last_error.store(error);
}

void set_error(DWORD error) {
    set_state(GuiderTransportState::Error, error);
}

bool valid_utf8(const std::string& text) {
    size_t index = 0;
    while (index < text.size()) {
        const auto first = static_cast<unsigned char>(text[index]);
        if (first <= 0x7F) {
            index += 1;
            continue;
        }
        size_t continuation_count = 0;
        uint32_t codepoint = 0;
        if (first >= 0xC2 && first <= 0xDF) {
            continuation_count = 1;
            codepoint = first & 0x1F;
        } else if (first >= 0xE0 && first <= 0xEF) {
            continuation_count = 2;
            codepoint = first & 0x0F;
        } else if (first >= 0xF0 && first <= 0xF4) {
            continuation_count = 3;
            codepoint = first & 0x07;
        } else {
            return false;
        }
        if (index + continuation_count >= text.size()) {
            return false;
        }
        for (size_t offset = 1; offset <= continuation_count; ++offset) {
            const auto next = static_cast<unsigned char>(text[index + offset]);
            if ((next & 0xC0) != 0x80) {
                return false;
            }
            codepoint = (codepoint << 6) | (next & 0x3F);
        }
        if ((continuation_count == 2 && codepoint < 0x800)
            || (continuation_count == 3 && codepoint < 0x10000)
            || codepoint > 0x10FFFF
            || (codepoint >= 0xD800 && codepoint <= 0xDFFF)) {
            return false;
        }
        index += continuation_count + 1;
    }
    return true;
}

void request_stop() {
    transport().stopping.store(true);
    transport().send_ready.notify_all();
    HINTERNET websocket = transport().websocket.load();
    if (websocket) {
        WinHttpWebSocketClose(websocket,
                              WINHTTP_WEB_SOCKET_SUCCESS_CLOSE_STATUS,
                              nullptr,
                              0);
    }
}

void sender_loop() {
    auto& value = transport();
    while (!value.stopping.load()) {
        std::string message;
        {
            std::unique_lock lock(value.mutex);
            value.send_ready.wait(lock, [&] {
                return value.stopping.load() || !value.send_queue.empty();
            });
            if (value.stopping.load()) {
                break;
            }
            message = std::move(value.send_queue.front());
            value.send_queue.pop_front();
        }
        const DWORD error = WinHttpWebSocketSend(
            value.websocket.load(),
            WINHTTP_WEB_SOCKET_UTF8_MESSAGE_BUFFER_TYPE,
            message.empty() ? nullptr : message.data(),
            static_cast<DWORD>(message.size()));
        if (error != ERROR_SUCCESS) {
            if (!value.stopping.load()) {
                set_error(error);
                request_stop();
            }
            break;
        }
    }
}

void receiver_loop() {
    auto& value = transport();
    std::string message;
    while (!value.stopping.load()) {
        std::array<char, kReceiveChunkSize> chunk{};
        DWORD bytes_read = 0;
        WINHTTP_WEB_SOCKET_BUFFER_TYPE buffer_type{};
        const DWORD error = WinHttpWebSocketReceive(
            value.websocket.load(),
            chunk.data(),
            static_cast<DWORD>(chunk.size()),
            &bytes_read,
            &buffer_type);
        if (error != ERROR_SUCCESS) {
            if (!value.stopping.load()) {
                set_state(GuiderTransportState::Closed, error);
                request_stop();
            }
            break;
        }
        message.append(chunk.data(), bytes_read);
        if (message.size() > value.configuration.max_frame_bytes) {
            value.overflowed.store(true);
            set_state(GuiderTransportState::Error, ERROR_INSUFFICIENT_BUFFER);
            request_stop();
            break;
        }
        if (buffer_type == WINHTTP_WEB_SOCKET_UTF8_MESSAGE_BUFFER_TYPE) {
            if (!valid_utf8(message)) {
                set_state(GuiderTransportState::Error, ERROR_INVALID_PARAMETER);
                request_stop();
                break;
            }
            {
                std::lock_guard lock(value.mutex);
                if (value.receive_queue.size() >= value.configuration.queue_limit) {
                    value.overflowed.store(true);
                    set_state(GuiderTransportState::Error, ERROR_INSUFFICIENT_BUFFER);
                    request_stop();
                    break;
                }
                value.receive_queue.push_back(std::move(message));
                value.receive_ready.notify_one();
            }
            message.clear();
        } else if (buffer_type == WINHTTP_WEB_SOCKET_CLOSE_BUFFER_TYPE) {
            set_state(GuiderTransportState::Closed);
            request_stop();
            break;
        }
    }
}

void connector_loop(uint16_t port, const std::string bearer_token) {
    auto& value = transport();
    HINTERNET session{};
    HINTERNET connection{};
    HINTERNET request{};
    HINTERNET websocket{};
    auto cleanup = [&] {
        if (request) WinHttpCloseHandle(request);
        if (connection) WinHttpCloseHandle(connection);
        if (session) WinHttpCloseHandle(session);
        if (websocket) WinHttpCloseHandle(websocket);
    };

    session = WinHttpOpen(L"GuiderTransport/0.1",
                          WINHTTP_ACCESS_TYPE_NO_PROXY,
                          WINHTTP_NO_PROXY_NAME,
                          WINHTTP_NO_PROXY_BYPASS,
                          0);
    if (!session) {
        set_error(GetLastError());
        return cleanup();
    }
    WinHttpSetTimeouts(session, 3000, 3000, 3000, 3000);
    connection = WinHttpConnect(session,
                                L"127.0.0.1",
                                port,
                                0);
    if (!connection) {
        set_error(GetLastError());
        return cleanup();
    }
    request = WinHttpOpenRequest(connection,
                                 L"GET",
                                 L"/",
                                 nullptr,
                                 nullptr,
                                 nullptr,
                                 0);
    if (!request) {
        set_error(GetLastError());
        return cleanup();
    }
    if (!WinHttpSetOption(request,
                          WINHTTP_OPTION_UPGRADE_TO_WEB_SOCKET,
                          nullptr,
                          0)) {
        set_error(GetLastError());
        return cleanup();
    }
    const std::wstring headers = L"Authorization: Bearer "
        + std::wstring(bearer_token.begin(), bearer_token.end()) + L"\r\n";
    if (!WinHttpSendRequest(request,
                            headers.c_str(),
                            static_cast<DWORD>(-1),
                            WINHTTP_NO_REQUEST_DATA,
                            0,
                            0,
                            0)) {
        DWORD error = GetLastError();
        DWORD status = 0;
        DWORD status_size = sizeof(status);
        WinHttpQueryHeaders(request,
                            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                            WINHTTP_HEADER_NAME_BY_INDEX,
                            &status,
                            &status_size,
                            WINHTTP_NO_HEADER_INDEX);
        if (status == 401) {
            set_state(GuiderTransportState::AuthFailed, status);
        } else {
            set_error(error);
        }
        return cleanup();
    }
    if (!WinHttpReceiveResponse(request, nullptr)) {
        set_error(GetLastError());
        return cleanup();
    }
    websocket = WinHttpWebSocketCompleteUpgrade(request, 0);
    if (!websocket) {
        DWORD error = GetLastError();
        DWORD status = 0;
        DWORD status_size = sizeof(status);
        WinHttpQueryHeaders(request,
                            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                            WINHTTP_HEADER_NAME_BY_INDEX,
                            &status,
                            &status_size,
                            WINHTTP_NO_HEADER_INDEX);
        if (status == 401) {
            set_state(GuiderTransportState::AuthFailed, status);
        } else {
            set_error(error);
        }
        return cleanup();
    }
    WinHttpCloseHandle(request);
    request = nullptr;

    value.websocket.store(websocket);
    websocket = nullptr;
    set_state(GuiderTransportState::Connected);
    value.sender = std::thread(sender_loop);
    value.receiver = std::thread(receiver_loop);
    {
        std::lock_guard lock(value.mutex);
        value.workers_started = true;
    }
    value.sender.join();
    value.receiver.join();
    WinHttpCloseHandle(value.websocket.load());
    value.websocket.store(nullptr);
    if (value.state.load() == GuiderTransportState::Connected) {
        set_state(GuiderTransportState::Closed);
    }
    cleanup();
}

} // namespace

extern "C" bool guider_transport_configure(uint16_t port,
                                        const char* bearer_token,
                                        size_t max_frame_bytes,
                                        size_t queue_limit) {
    if (port == 0 || bearer_token == nullptr || bearer_token[0] == '\0'
        || max_frame_bytes == 0 || queue_limit == 0) {
        return false;
    }
    auto& value = transport();
    std::lock_guard lock(value.mutex);
    value.configuration.port = port;
    value.configuration.bearer_token = bearer_token;
    value.configuration.max_frame_bytes = max_frame_bytes;
    value.configuration.queue_limit = queue_limit;
    value.send_queue.clear();
    value.receive_queue.clear();
    value.overflowed.store(false);
    value.last_error.store(0);
    value.state.store(GuiderTransportState::Idle);
    return true;
}

extern "C" bool guider_transport_connect() {
    auto& value = transport();
    const auto current = value.state.load();
    if (current != GuiderTransportState::Idle
        && current != GuiderTransportState::AuthFailed
        && current != GuiderTransportState::Closed
        && current != GuiderTransportState::Error) {
        return false;
    }
    if (value.connector.joinable()) {
        request_stop();
        value.connector.join();
    }
    value.stopping.store(false);
    value.workers_started = false;
    value.websocket.store(nullptr);
    value.send_queue.clear();
    value.receive_queue.clear();
    value.overflowed.store(false);
    value.last_error.store(0);
    set_state(GuiderTransportState::Connecting);
    value.connector = std::thread(connector_loop,
                                  value.configuration.port,
                                  value.configuration.bearer_token);
    return true;
}

extern "C" bool guider_transport_send(const char* text, size_t byte_count) {
    if (text == nullptr && byte_count != 0) {
        return false;
    }
    auto& value = transport();
    if (value.state.load() != GuiderTransportState::Connected
        || value.stopping.load()) {
        return false;
    }
    std::string message(text == nullptr ? "" : text, byte_count);
    if (!valid_utf8(message)
        || message.size() > value.configuration.max_frame_bytes) {
        return false;
    }
    std::lock_guard lock(value.mutex);
    if (value.send_queue.size() >= value.configuration.queue_limit) {
        value.overflowed.store(true);
        set_state(GuiderTransportState::Error, ERROR_INSUFFICIENT_BUFFER);
        request_stop();
        return false;
    }
    value.send_queue.push_back(std::move(message));
    value.send_ready.notify_one();
    return true;
}

extern "C" bool guider_transport_poll(int32_t timeout_ms,
                                   char* output,
                                   size_t output_capacity,
                                   size_t* output_byte_count) {
    if (output == nullptr || output_capacity == 0 || output_byte_count == nullptr) {
        return false;
    }
    auto& value = transport();
    std::unique_lock lock(value.mutex);
    const bool received = value.receive_ready.wait_for(
        lock,
        std::chrono::milliseconds(timeout_ms < 0 ? 0 : timeout_ms),
        [&] { return !value.receive_queue.empty(); });
    if (!received) {
        return false;
    }
    std::string message = std::move(value.receive_queue.front());
    value.receive_queue.pop_front();
    if (message.size() + 1 > output_capacity) {
        return false;
    }
    std::memcpy(output, message.data(), message.size());
    output[message.size()] = '\0';
    *output_byte_count = message.size();
    return true;
}

extern "C" const char* guider_transport_status() {
    switch (transport().state.load()) {
        case GuiderTransportState::Idle: return "idle";
        case GuiderTransportState::Connecting: return "connecting";
        case GuiderTransportState::Connected: return "connected";
        case GuiderTransportState::AuthFailed: return "auth_failed";
        case GuiderTransportState::Closed: return "closed";
        case GuiderTransportState::Error: return "error";
    }
    return "error";
}

extern "C" uint32_t guider_transport_last_error() {
    return transport().last_error.load();
}

extern "C" bool guider_transport_close() {
    auto& value = transport();
    if (value.state.load() == GuiderTransportState::Idle) {
        return true;
    }
    request_stop();
    value.receive_ready.notify_all();
    return true;
}

extern "C" void guider_transport_shutdown() {
    auto& value = transport();
    request_stop();
    value.receive_ready.notify_all();
    if (value.sender.joinable()) value.sender.join();
    if (value.receiver.joinable()) value.receiver.join();
    if (value.connector.joinable()) value.connector.join();
}
