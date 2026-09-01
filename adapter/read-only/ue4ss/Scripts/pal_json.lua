local PalJSON = {}

local function json_escape(value)
    local result = {}
    for index = 1, #value do
        local byte = value:byte(index)
        if byte == 34 then
            result[#result + 1] = "\\\""
        elseif byte == 92 then
            result[#result + 1] = "\\\\"
        elseif byte == 8 then
            result[#result + 1] = "\\b"
        elseif byte == 9 then
            result[#result + 1] = "\\t"
        elseif byte == 10 then
            result[#result + 1] = "\\n"
        elseif byte == 13 then
            result[#result + 1] = "\\r"
        elseif byte < 32 or byte == 127 then
            result[#result + 1] = string.format("\\u%04x", byte)
        else
            result[#result + 1] = value:sub(index, index)
        end
    end
    return table.concat(result)
end

local function table_is_array(value)
    return #value > 0 or next(value) == nil
end

function PalJSON.encode(value)
    if value == nil then
        return "null"
    end
    if type(value) == "boolean" then
        return tostring(value)
    end
    if type(value) == "number" then
        if value ~= value or value == math.huge or value == -math.huge then
            return "null"
        end
        return string.format("%.17g", value)
    end
    if type(value) == "string" then
        return "\"" .. json_escape(value) .. "\""
    end
    if type(value) ~= "table" then
        return "null"
    end
    if table_is_array(value) then
        local items = {}
        for index = 1, #value do
            items[#items + 1] = PalJSON.encode(value[index])
        end
        return "[" .. table.concat(items, ",") .. "]"
    end
    local members = {}
    for key, item in pairs(value) do
        members[#members + 1] = "\"" .. json_escape(tostring(key)) .. "\":" .. PalJSON.encode(item)
    end
    table.sort(members)
    return "{" .. table.concat(members, ",") .. "}"
end

local function unicode_escape(code)
    if code >= 0x10000 then
        local base = code - 0x10000
        local high = 0xD800 + math.floor(base / 0x400)
        local low = 0xDC00 + base % 0x400
        return string.format("\\u%04x\\u%04x", high, low)
    end
    return string.format("\\u%04x", code)
end

local function parse_string(text, start)
    if text:sub(start, start) ~= "\"" then
        return nil, start
    end
    local position = start + 1
    local result = {}
    while position <= #text do
        local character = text:sub(position, position)
        if character == "\"" then
            return table.concat(result), position + 1
        elseif character == "\\" then
            local escape = text:sub(position + 1, position + 1)
            if escape == "u" then
                local digits = text:sub(position + 2, position + 5)
                local code = tonumber(digits, 16)
                if not code then
                    return nil, start
                end
                local encoded
                if utf8 and utf8.char then
                    local ok, value = pcall(utf8.char, code)
                    encoded = ok and value or unicode_escape(code)
                else
                    encoded = unicode_escape(code)
                end
                result[#result + 1] = encoded
                position = position + 6
            else
                local decoded = {b="\b", f="\f", n="\n", r="\r", t="\t", ['"']='"', ["\\"]="\\", ["/"]="/"}
                result[#result + 1] = decoded[escape] or escape
                position = position + 2
            end
        else
            result[#result + 1] = character
            position = position + 1
        end
    end
    return nil, start
end

local function skip_whitespace(text, position)
    while position <= #text and text:sub(position, position):match("[%s]") do
        position = position + 1
    end
    return position
end

local parse_value

local function parse_object(text, position)
    local result = {}
    position = skip_whitespace(text, position + 1)
    if text:sub(position, position) == "}" then
        return result, position + 1
    end
    while true do
        position = skip_whitespace(text, position)
        local key, next_position = parse_string(text, position)
        if not key then
            return nil, position
        end
        position = skip_whitespace(text, next_position)
        if text:sub(position, position) ~= ":" then
            return nil, position
        end
        local value
        value, position = parse_value(text, skip_whitespace(text, position + 1))
        if value == nil and text:sub(position, position):find("[ntf%[%{\"]") then
            return nil, position
        end
        result[key] = value
        position = skip_whitespace(text, position)
        local separator = text:sub(position, position)
        if separator == "," then
            position = position + 1
        elseif separator == "}" then
            return result, position + 1
        else
            return nil, position
        end
    end
end

local function parse_array(text, position)
    local result = {}
    position = skip_whitespace(text, position + 1)
    if text:sub(position, position) == "]" then
        return result, position + 1
    end
    while true do
        local value
        value, position = parse_value(text, position)
        if value == nil and text:sub(position, position):find("[ntf%[%{\"]") then
            return nil, position
        end
        result[#result + 1] = value
        position = skip_whitespace(text, position)
        local separator = text:sub(position, position)
        if separator == "," then
            position = position + 1
        elseif separator == "]" then
            return result, position + 1
        else
            return nil, position
        end
    end
end

parse_value = function(text, position)
    position = skip_whitespace(text, position)
    local character = text:sub(position, position)
    if character == "{" then
        return parse_object(text, position)
    elseif character == "[" then
        return parse_array(text, position)
    elseif character == "\"" then
        return parse_string(text, position)
    elseif text:sub(position, position + 3) == "true" then
        return true, position + 4
    elseif text:sub(position, position + 4) == "false" then
        return false, position + 5
    elseif text:sub(position, position + 3) == "null" then
        return nil, position + 4
    end
    local number = text:match("^-?%d+%.?%d*[eE]?[-+]?%d*", position)
    if number then
        return tonumber(number), position + #number
    end
    return nil, position
end

function PalJSON.decode(text)
    local value, position = parse_value(text, 1)
    if value == nil and skip_whitespace(text, position) <= #text then
        return nil
    end
    if skip_whitespace(text, position) <= #text then
        return nil
    end
    return value
end

return PalJSON
