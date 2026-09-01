-- Palworld Guider: read-only UE4SS Lua adapter.
--
-- Registers the validated chat-input hook, publishes prefixed chat
-- questions as schema-2 events, and answers whitelisted read-only tool
-- calls. The native shim owns the authenticated loopback WebSocket
-- transport; this file contains no file IPC, game mutation, chat-body
-- logging, or advisory logic.

local UEHelpers = require("UEHelpers")

local SCHEMA_VERSION = 2
local TAG = "[PalworldGuider]"
local COMMAND_PREFIX = "!g "
local MAX_CHAT_MESSAGE_CHARS = 500
local EVENT_NONCE = tostring(os.clock()):gsub("%.", "-", 1)
local EVENT_SEQUENCE = 0
local PalTransport = require("pal_transport")

local function valid_object(object)
    if object == nil then
        return false
    end
    local ok, valid = pcall(function()
        return object:IsValid()
    end)
    return ok and valid == true
end

local function get_world()
    local world
    pcall(function()
        world = UEHelpers.GetWorld()
    end)
    return valid_object(world) and world or nil
end

local function get_player_controller()
    local controller
    pcall(function()
        controller = UEHelpers.GetPlayerController()
    end)
    return valid_object(controller) and controller or nil
end

local function get_pal_utility()
    local utility
    pcall(function()
        utility = StaticFindObject("/Script/Pal.Default__PalUtility")
    end)
    return valid_object(utility) and utility or nil
end

local function finite_number(value)
    return type(value) == "number"
        and value == value
        and value ~= math.huge
        and value ~= -math.huge
end

local function chat_trace(stage, chars, prefix)
    print(TAG .. " chat_probe stage=" .. tostring(stage)
        .. " chars=" .. tostring(chars)
        .. " prefix=" .. tostring(prefix))
end

local function position_object(actor)
    local location
    local ok = pcall(function()
        location = actor:K2_GetActorLocation()
    end)
    if not ok or location == nil then
        return nil
    end
    local x, y, z
    pcall(function()
        x = tonumber(location.X)
        y = tonumber(location.Y)
        z = tonumber(location.Z)
    end)
    if not (finite_number(x) and finite_number(y) and finite_number(z)) then
        return nil
    end
    return {x = x, y = y, z = z}
end

local function send_chat(world, message)
    local utility = get_pal_utility()
    if not utility or not world then
        return false, "chat output unavailable"
    end
    local ok, error = pcall(function()
        utility:SendSystemToPlayerChat(world, message, {})
    end)
    if not ok then
        return false, tostring(error)
    end
    return true, nil
end

local function response_frame(call_id, status, data, error)
    return {
        schema_version = SCHEMA_VERSION,
        type = "tool_result",
        call_id = call_id,
        seq = PalTransport.next_sequence(),
        timestamp_ms = os.time() * 1000,
        correlation_id = call_id,
        status = status,
        data = data or {},
        error = error,
        elapsed_ms = 0,
    }
end

local function write_response(frame)
    local sent = PalTransport.send_result(frame)
    if not sent then
        print(TAG .. " tool result send failed status=" .. tostring(frame.status))
    end
    return sent
end
local function handle_player_status(call_id)
    local world = get_world()
    local controller = get_player_controller()
    local pawn
    if controller then
        pcall(function()
            pawn = controller.Pawn
        end)
    end
    if not world or not controller or not valid_object(pawn) then
        write_response(response_frame(call_id, "unavailable", nil, "world, controller, or pawn unavailable"))
        return
    end
    local position = position_object(pawn)
    if not position then
        write_response(response_frame(call_id, "unavailable", nil, "player position unavailable"))
        return
    end
    write_response(response_frame(call_id, "ok", {
        controller_valid = true,
        pawn_valid = true,
        position = position,
    }, nil))
end

local function get_active_otomo(world)
    local utility = get_pal_utility()
    if not utility then
        return nil, "Pal utility unavailable"
    end

    local controller
    local controller_ok = pcall(function()
        controller = utility:GetLocalPalPlayerController(world)
    end)
    if not controller_ok then
        return nil, "native error reading local Pal controller"
    end
    if not valid_object(controller) then
        return nil, "local Pal controller unavailable"
    end

    local holder
    local holder_ok = pcall(function()
        holder = utility:GetOtomoHolderComponent(controller)
    end)
    if not holder_ok then
        return nil, "native error reading Otomo holder"
    end
    if not valid_object(holder) then
        return nil, "Otomo holder unavailable"
    end

    local handle
    local handle_ok = pcall(function()
        handle = holder:TryGetSpawnedOtomoHandle()
    end)
    if not handle_ok then
        return nil, "native error reading active Otomo handle"
    end
    if not valid_object(handle) then
        return nil, "active Otomo handle unavailable"
    end

    local parameter
    local parameter_ok = pcall(function()
        parameter = handle:TryGetIndividualParameter()
    end)
    if not parameter_ok then
        return nil, "native error reading Otomo individual parameter"
    end
    if not valid_object(parameter) then
        return nil, "Otomo individual parameter unavailable"
    end

    local character_id
    local character_ok = pcall(function()
        character_id = parameter:GetCharacterID():ToString()
    end)
    if not character_ok then
        return nil, "Otomo character identity unavailable"
    end
    if type(character_id) ~= "string" or character_id:match("^%s*$") then
        return nil, "Otomo character identity unavailable"
    end

    local actor
    local actor_ok = pcall(function()
        actor = handle:TryGetIndividualActor()
    end)
    if not actor_ok then
        return nil, "native error reading Otomo actor"
    end
    if not valid_object(actor) then
        return nil, "Otomo actor unavailable"
    end

    local position = position_object(actor)
    if not position then
        return nil, "Otomo position unavailable"
    end

    return {
        available = true,
        character_id = character_id,
        actor_valid = true,
        position = position,
    }, nil
end
local function handle_active_pal_status(call_id)
    local world = get_world()
    if not world then
        write_response(response_frame(call_id, "unavailable", nil, "world unavailable"))
        return
    end
    local status, error = get_active_otomo(world)
    if not status then
        write_response(response_frame(call_id, "unavailable", nil, error))
        return
    end
    write_response(response_frame(call_id, "ok", status, nil))
end

local function handle_send_chat(call_id, args)
    local message = type(args) == "table" and args.message
    if type(message) ~= "string"
        or message == ""
        or message:match("^%s*$")
        or #message > MAX_CHAT_MESSAGE_CHARS then
        write_response(response_frame(call_id, "invalid_args", nil, "chat message must be 1-500 characters"))
        return
    end
    local world = get_world()
    if not world then
        write_response(response_frame(call_id, "unavailable", nil, "world unavailable"))
        return
    end
    local sent, error = send_chat(world, message)
    if not sent then
        write_response(response_frame(call_id, "unavailable", nil, error))
        return
    end
    write_response(response_frame(call_id, "ok", {
        native_call_accepted = true,
        player_visible_receipt = false,
    }, nil))
end

local function handle_request(frame)
    if type(frame) ~= "table" or frame.schema_version ~= SCHEMA_VERSION or frame.type ~= "tool_call" then
        return
    end
    local call_id = frame.call_id
    if type(call_id) ~= "string" or call_id == "" then
        return
    end
    if frame.tool == "ping" then
        write_response(response_frame(call_id, "ok", {service = "palworld-guider"}, nil))
    elseif frame.tool == "get_player_status" then
        handle_player_status(call_id)
    elseif frame.tool == "get_active_pal_status" then
        handle_active_pal_status(call_id)
    elseif frame.tool == "send_chat_message" then
        handle_send_chat(call_id, frame.args)
    else
        write_response(response_frame(call_id, "unsupported", nil, "tool is not whitelisted"))
    end
end

local function dispatch_request_to_game_thread(frame)
    ExecuteInGameThread(function()
        handle_request(frame)
    end)
end

local function publish_chat_event(text)
    EVENT_SEQUENCE = EVENT_SEQUENCE + 1
    local sequence_text = string.format("%020d", EVENT_SEQUENCE)
    local event_id = EVENT_NONCE .. "-" .. tostring(os.time()) .. "-" .. sequence_text
    PalTransport.send_event(text, event_id)
end

local function poll_transport()
    PalTransport.poll(dispatch_request_to_game_thread, 0)
end

if not PalTransport.connect() then
    print(TAG .. " websocket transport configuration unavailable")
end

local chat_hook_registered = false

local function chat_hook_callback(_, parameter)
    if not chat_hook_registered then
        return
    end
    local unregistered, unregister_error = pcall(
        UnregisterHook,
        "/Script/Pal.PalGameStateInGame:BroadcastChatMessage",
        chat_hook_callback
    )
    if unregistered then
        chat_hook_registered = false
    else
        print(TAG .. " chat hook unregister failed: " .. tostring(unregister_error))
    end
    chat_trace("callback", -1, false)
    local message
    local ok = pcall(function()
        local raw = parameter and parameter:get()
        message = raw and raw.Message and raw.Message:ToString()
    end)
    if not ok then
        chat_trace("parameter-error", -1, false)
        return
    end
    local message_chars = type(message) == "string" and #message or -1
    local prefix_matched = type(message) == "string"
        and #message >= #COMMAND_PREFIX
        and message:sub(1, #COMMAND_PREFIX) == COMMAND_PREFIX
    chat_trace("received", message_chars, prefix_matched)
    if ok and type(message) == "string" and message:sub(1, #COMMAND_PREFIX) == COMMAND_PREFIX then
        local instruction = message:sub(#COMMAND_PREFIX + 1):match("^%s*(.-)%s*$")
        if instruction ~= "" then
            chat_trace("accepted", #message, true)
            publish_chat_event(instruction)
        end
    end
end

local function ensure_chat_hook()
    if chat_hook_registered then
        return
    end
    local registered, register_error = pcall(
        RegisterHook,
        "/Script/Pal.PalGameStateInGame:BroadcastChatMessage",
        chat_hook_callback
    )
    chat_hook_registered = registered
    if not registered then
        print(TAG .. " chat hook registration failed: " .. tostring(register_error))
    end
end

ensure_chat_hook()
print(TAG .. " loaded; use " .. COMMAND_PREFIX .. "<question> in chat")
LoopAsync(50, function()
    poll_transport()
    ensure_chat_hook()
    return false
end)
