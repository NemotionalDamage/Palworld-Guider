#pragma once

#include <File/Macros.hpp>

#include <memory>
#include <vector>

namespace RC
{
    namespace LuaMadeSimple
    {
        class Lua;
    }

    namespace GUI
    {
        class GUITab;
    }

    class CppUserModBase
    {
      protected:
        std::vector<std::shared_ptr<GUI::GUITab>> GUITabs{};

      public:
        StringType ModName{};
        StringType ModVersion{};
        StringType ModDescription{};
        StringType ModAuthors{};
        StringType ModIntendedSDKVersion{};

      public:
        RC_UE4SS_API CppUserModBase();
        RC_UE4SS_API virtual ~CppUserModBase();

      public:
        RC_UE4SS_API virtual auto on_update() -> void;
        RC_UE4SS_API virtual auto on_unreal_init() -> void;
        RC_UE4SS_API virtual auto on_program_start() -> void;
        RC_UE4SS_API virtual auto on_lua_start(StringViewType mod_name,
                                               LuaMadeSimple::Lua& lua,
                                               LuaMadeSimple::Lua& main_lua,
                                               LuaMadeSimple::Lua& async_lua,
                                               std::vector<LuaMadeSimple::Lua*>& hook_luas) -> void;
        RC_UE4SS_API virtual auto on_lua_start(LuaMadeSimple::Lua& lua,
                                               LuaMadeSimple::Lua& main_lua,
                                               LuaMadeSimple::Lua& async_lua,
                                               std::vector<LuaMadeSimple::Lua*>& hook_luas) -> void;
        RC_UE4SS_API virtual auto on_lua_stop(StringViewType mod_name,
                                              LuaMadeSimple::Lua& lua,
                                              LuaMadeSimple::Lua& main_lua,
                                              LuaMadeSimple::Lua& async_lua,
                                              std::vector<LuaMadeSimple::Lua*>& hook_luas) -> void;
        RC_UE4SS_API virtual auto on_lua_stop(LuaMadeSimple::Lua& lua,
                                              LuaMadeSimple::Lua& main_lua,
                                              LuaMadeSimple::Lua& async_lua,
                                              std::vector<LuaMadeSimple::Lua*>& hook_luas) -> void;
        RC_UE4SS_API virtual auto on_dll_load(StringViewType dll_name) -> void;
        RC_UE4SS_API virtual auto render_tab() -> void;

      protected:
        RC_UE4SS_API auto register_tab(StringViewType tab_name,
                                      void (*render_function)(CppUserModBase*)) -> void;
    };
}

static_assert(sizeof(RC::CppUserModBase) == 192, "UE4SS 3.0.1 CppUserModBase ABI changed");
