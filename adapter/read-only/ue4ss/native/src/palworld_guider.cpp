// Palworld Guider: transport-only UE4SS lifecycle shim.
//
// Registers exactly the `guider_*` Lua callbacks that bridge to the
// WinHTTP WebSocket carrier. This file contains no Unreal or Palworld
// API, hook, shell, file-path, provider, or advisory logic.

#include <Mod/CppUserModBase.hpp>

#include "pal_transport.h"

#include <algorithm>
#include <cstdint>
#include <new>
#include <string>
#include <string_view>
#include <vector>

static_assert(sizeof(RC::CppUserModBase) == 192, "UE4SS 3.0.1 CppUserModBase ABI changed");

namespace RC::LuaMadeSimple
{
    class __declspec(dllimport) Lua
    {
      public:
        using LuaFunction = int (*)(const Lua&);

      public:
        auto get_integer(int32_t force_index = 1) const -> int64_t;
        auto get_string(int32_t force_index = 1) const -> std::string_view;
        auto register_function(const std::string& name, const LuaFunction& function) const -> void;
        auto set_string(std::string_view value) const -> void;
    };
}

namespace
{
constexpr size_t kMaxLuaFrameBytes = 65536;

auto guider_cfg(const RC::LuaMadeSimple::Lua& lua) -> int
{
    const int64_t queue_limit = lua.get_integer(4);
    const int64_t max_frame = lua.get_integer(3);
    const std::string_view token_view = lua.get_string(2);
    const std::string token(token_view);
    const int64_t port = lua.get_integer(1);
    const bool configured = port >= 1 && port <= 65535
        && !token.empty() && token.size() <= 4096
        && std::all_of(token.begin(), token.end(), [](unsigned char character) {
               return character > 0x20 && character != 0x7f;
           })
        && max_frame >= 1 && max_frame <= 65536
        && queue_limit >= 1 && queue_limit <= 64
        && guider_transport_configure(static_cast<uint16_t>(port),
                                      token.data(),
                                      static_cast<size_t>(max_frame),
                                      static_cast<size_t>(queue_limit));
    lua.set_string(configured ? "true" : "false");
    return 1;
}

auto guider_conn(const RC::LuaMadeSimple::Lua& lua) -> int
{
    lua.set_string(guider_transport_connect() ? "true" : "false");
    return 1;
}

auto guider_send(const RC::LuaMadeSimple::Lua& lua) -> int
{
    const std::string_view text = lua.get_string(1);
    const bool sent = text.size() >= 1
        && text.size() <= kMaxLuaFrameBytes
        && guider_transport_send(text.data(), static_cast<size_t>(text.size()));
    lua.set_string(sent ? "true" : "false");
    return 1;
}

auto guider_poll(const RC::LuaMadeSimple::Lua& lua) -> int
{
    const int64_t timeout_ms = lua.get_integer(1);
    if (timeout_ms < 0 || timeout_ms > 1000)
    {
        lua.set_string("");
        return 1;
    }

    std::vector<char> receive_buffer(kMaxLuaFrameBytes + 1);
    size_t byte_count = 0;
    const bool received = guider_transport_poll(static_cast<int32_t>(timeout_ms),
                                                receive_buffer.data(),
                                                receive_buffer.size(),
                                                &byte_count);
    lua.set_string(received ? std::string_view(receive_buffer.data(), byte_count) : "");
    return 1;
}

auto guider_stat(const RC::LuaMadeSimple::Lua& lua) -> int
{
    lua.set_string(guider_transport_status());
    return 1;
}

auto guider_close(const RC::LuaMadeSimple::Lua& lua) -> int
{
    lua.set_string(guider_transport_close() ? "true" : "false");
    return 1;
}

class PalworldGuiderMod : public RC::CppUserModBase
{
  public:
    PalworldGuiderMod() : RC::CppUserModBase()
    {
        ModName = STR("PalworldGuider");
        ModVersion = STR("0.1.0");
        ModDescription = STR("Read-only loopback transport carrier for the Palworld Guider");
        ModAuthors = STR("Palworld Guider");
        ModIntendedSDKVersion = STR("3.0.1");
    }

    ~PalworldGuiderMod() override
    {
        guider_transport_shutdown();
    }

    auto on_update() -> void override
    {
    }

    auto on_program_start() -> void override
    {
    }

    auto on_unreal_init() -> void override
    {
    }

    auto on_lua_start(RC::StringViewType,
                      RC::LuaMadeSimple::Lua& lua,
                      RC::LuaMadeSimple::Lua&,
                      RC::LuaMadeSimple::Lua&,
                      std::vector<RC::LuaMadeSimple::Lua*>&) -> void override
    {
        lua.register_function("guider_cfg", &guider_cfg);
        lua.register_function("guider_conn", &guider_conn);
        lua.register_function("guider_send", &guider_send);
        lua.register_function("guider_poll", &guider_poll);
        lua.register_function("guider_stat", &guider_stat);
        lua.register_function("guider_close", &guider_close);
    }

    auto on_lua_start(RC::LuaMadeSimple::Lua&,
                      RC::LuaMadeSimple::Lua&,
                      RC::LuaMadeSimple::Lua&,
                      std::vector<RC::LuaMadeSimple::Lua*>&) -> void override
    {
    }

    auto on_lua_stop(RC::StringViewType,
                     RC::LuaMadeSimple::Lua&,
                     RC::LuaMadeSimple::Lua&,
                     RC::LuaMadeSimple::Lua&,
                     std::vector<RC::LuaMadeSimple::Lua*>&) -> void override
    {
        guider_transport_shutdown();
    }

    auto on_lua_stop(RC::LuaMadeSimple::Lua&,
                     RC::LuaMadeSimple::Lua&,
                     RC::LuaMadeSimple::Lua&,
                     std::vector<RC::LuaMadeSimple::Lua*>&) -> void override
    {
        guider_transport_shutdown();
    }

    auto on_dll_load(RC::StringViewType) -> void override
    {
    }

    auto render_tab() -> void override
    {
    }
};
}

extern "C" RC::CppUserModBase* start_mod()
{
    return new (std::nothrow) PalworldGuiderMod();
}

extern "C" void uninstall_mod(RC::CppUserModBase* mod)
{
    delete mod;
}
