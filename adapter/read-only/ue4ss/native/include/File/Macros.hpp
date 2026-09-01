#pragma once

#include <string>

#ifndef RC_UE4SS_API
#define RC_UE4SS_API __declspec(dllimport)
#endif

#ifndef STR
#define STR(value) L##value
#endif

namespace RC
{
    using StringType = std::wstring;
    using StringViewType = std::wstring_view;
}
