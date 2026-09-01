#ifndef GUIDER_TRANSPORT_H
#define GUIDER_TRANSPORT_H

#include <cstddef>
#include <cstdint>

enum class GuiderTransportState {
    Idle,
    Connecting,
    Connected,
    AuthFailed,
    Closed,
    Error,
};

extern "C" {
bool guider_transport_configure(uint16_t port,
                                const char* bearer_token,
                                size_t max_frame_bytes,
                                size_t queue_limit);
bool guider_transport_connect();
bool guider_transport_send(const char* text, size_t byte_count);
bool guider_transport_poll(int32_t timeout_ms,
                           char* output,
                           size_t output_capacity,
                           size_t* output_byte_count);
const char* guider_transport_status();
uint32_t guider_transport_last_error();
bool guider_transport_close();
void guider_transport_shutdown();
}

#endif
