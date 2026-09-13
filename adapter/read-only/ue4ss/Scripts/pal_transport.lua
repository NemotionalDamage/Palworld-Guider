-- Palworld Guider: schema-2 session state over the native loopback
-- transport. Bounded queues, capped exponential reconnect, heartbeat,
-- and strict sequence validation live here; the native shim only moves
-- authenticated UTF-8 text frames.

local JSON = require("pal_json")

local SCHEMA_VERSION = 2
local RECONNECT_DELAY_MS = 250
local MAX_RECONNECT_DELAY_MS = 30000
local HEARTBEAT_INTERVAL_MS = 15000
local DEFAULT_PORT = 8071
local MAX_FRAME_BYTES = 65536
local QUEUE_LIMIT = 64
local TOKEN_MIN_CHARS = 16
local TOKEN_MAX_CHARS = 4096

local GuiderTransport = {}
local configured = false
local greeted = false
local outbound_sequence = 0
local expected_sequence = 0
local reconnect_delay_ms = RECONNECT_DELAY_MS
local reconnect_at_ms = 0
local last_heartbeat_ms = 0
local last_reported_status = "unreported"

local function now_ms()
    return os.time() * 1000
end

local function reset_session()
    outbound_sequence = 0
    expected_sequence = 0
    greeted = false
    last_heartbeat_ms = now_ms()
end

local function log_transport_event(event, status, accepted)
    local accepted_text = "none"
    if accepted ~= nil then
        accepted_text = tostring(accepted)
    end
    print(string.format(
        "[GuiderTransport] event=%s status=%s accepted=%s",
        event,
        status,
        accepted_text
    ))
end

function GuiderTransport.next_sequence()
    outbound_sequence = outbound_sequence + 1
    return outbound_sequence
end

local function send_frame(frame)
    if not greeted then
        return false
    end
    local ok = guider_send(JSON.encode(frame))
    if not ok then
        greeted = false
        guider_close()
        if reconnect_at_ms == 0 then
            reconnect_at_ms = now_ms() + reconnect_delay_ms
            reconnect_delay_ms = math.min(reconnect_delay_ms * 2, MAX_RECONNECT_DELAY_MS)
        end
    end
    return ok
end

function GuiderTransport.send_frame(frame)
    return send_frame(frame)
end

local function hello_frame(sequence)
    return {
        schema_version = SCHEMA_VERSION,
        type = "hello",
        seq = sequence,
        timestamp_ms = now_ms(),
    }
end

function GuiderTransport.capability_manifest_frame(sequence)
    return {
        schema_version = SCHEMA_VERSION,
        type = "capability_manifest",
        seq = sequence,
        timestamp_ms = now_ms(),
        data = {
            tools = {
                {name = "ping", mutation = false, evidence = "A", status = "enabled"},
                {name = "get_player_status", mutation = false, evidence = "A", status = "enabled"},
                {name = "get_active_pal_status", mutation = false, evidence = "A", status = "enabled"},
                {name = "get_base_camps", mutation = false, evidence = "A", status = "enabled"},
                {name = "send_chat_message", mutation = true, evidence = "A", status = "enabled"},
            },
        },
    }
end

function GuiderTransport.heartbeat_frame(sequence)
    return {
        schema_version = SCHEMA_VERSION,
        type = "heartbeat",
        seq = sequence,
        timestamp_ms = now_ms(),
    }
end

local function send_hello_and_manifest()
    reset_session()
    greeted = true
    local hello_sent = send_frame(hello_frame(GuiderTransport.next_sequence()))
    local manifest_sent = send_frame(GuiderTransport.capability_manifest_frame(GuiderTransport.next_sequence()))
    greeted = hello_sent and manifest_sent
    if not greeted then
        return false
    end
    return true
end
function GuiderTransport.configure()
    local token = os.getenv("PALWORLD_GUIDER_GATEWAY_TOKEN")
    local port = tonumber(os.getenv("PALWORLD_GUIDER_GATEWAY_PORT") or tostring(DEFAULT_PORT))
    if type(token) ~= "string"
        or #token < TOKEN_MIN_CHARS
        or #token > TOKEN_MAX_CHARS
        or token:find("[%c]") then
        return false
    end
    if type(port) ~= "number" or port < 1 or port > 65535 or port % 1 ~= 0 then
        return false
    end
    configured = guider_cfg(port, token, MAX_FRAME_BYTES, QUEUE_LIMIT)
    reset_session()
    return configured
end

function GuiderTransport.connect()
    if not configured and not GuiderTransport.configure() then
        return false
    end
    reset_session()
    reconnect_at_ms = 0
    reconnect_delay_ms = RECONNECT_DELAY_MS
    return guider_conn()
end

function GuiderTransport.status()
    return guider_stat()
end

function GuiderTransport.close()
    return guider_close()
end

function GuiderTransport.validate_inbound_sequence(frame, sequence)
    if type(frame) ~= "table"
        or frame.schema_version ~= SCHEMA_VERSION
        or type(frame.type) ~= "string"
        or type(frame.timestamp_ms) ~= "number" then
        return false
    end
    if sequence == nil then
        sequence = frame.seq
    end
    if type(sequence) ~= "number"
        or sequence % 1 ~= 0
        or sequence <= 0
        or sequence ~= expected_sequence + 1 then
        return false
    end
    expected_sequence = sequence
    return true
end

function GuiderTransport.send_event(text, event_id)
    if type(text) ~= "string" or text == "" then
        return false
    end
    return send_frame({
        schema_version = SCHEMA_VERSION,
        type = "event",
        event = "chat_message",
        event_id = event_id or ("lua-" .. tostring(os.time()) .. "-" .. tostring(outbound_sequence + 1)),
        seq = GuiderTransport.next_sequence(),
        timestamp_ms = now_ms(),
        source = "client",
        data = {player_id = "local-player", text = text},
    })
end

function GuiderTransport.send_result(frame)
    if type(frame) ~= "table" then
        return false
    end
    if frame.seq == nil then
        frame.seq = GuiderTransport.next_sequence()
    end
    if frame.timestamp_ms == nil then
        frame.timestamp_ms = now_ms()
    end
    return send_frame(frame)
end

local function malformed_inbound()
    greeted = false
    guider_close()
    if reconnect_at_ms == 0 then
        reconnect_at_ms = now_ms() + reconnect_delay_ms
        reconnect_delay_ms = math.min(reconnect_delay_ms * 2, MAX_RECONNECT_DELAY_MS)
    end
end

function GuiderTransport.poll(handle_tool_call, timeout_ms)
    if type(handle_tool_call) ~= "function" then
        return false
    end
    local status = guider_stat()
    if status ~= last_reported_status then
        last_reported_status = status
        log_transport_event("status_changed", status, nil)
    end
    if status == "connected" then
        if not greeted then
            reconnect_delay_ms = RECONNECT_DELAY_MS
            send_hello_and_manifest()
            return true
        end
        if now_ms() - last_heartbeat_ms >= HEARTBEAT_INTERVAL_MS then
            last_heartbeat_ms = now_ms()
            local accepted = send_frame(
                GuiderTransport.heartbeat_frame(GuiderTransport.next_sequence())
            )
            log_transport_event("heartbeat", GuiderTransport.status(), accepted)
        end
    elseif status == "closed" or status == "error" or status == "auth_failed" then
        local current_time = now_ms()
        if reconnect_at_ms == 0 then
            reconnect_at_ms = current_time + reconnect_delay_ms
            reconnect_delay_ms = math.min(reconnect_delay_ms * 2, MAX_RECONNECT_DELAY_MS)
        elseif current_time >= reconnect_at_ms then
            reconnect_at_ms = 0
            reset_session()
            guider_conn()
        end
        return true
    end

    local text = guider_poll(timeout_ms or 0)
    if not text or text == "" then
        return true
    end
    local ok, frame = pcall(JSON.decode, text)
    if not ok or not GuiderTransport.validate_inbound_sequence(frame) then
        malformed_inbound()
        return true
    end
    if frame.type ~= "tool_call"
        or type(frame.call_id) ~= "string"
        or frame.call_id == ""
        or type(frame.tool) ~= "string" then
        malformed_inbound()
        return true
    end
    handle_tool_call(frame)
    return true
end

return GuiderTransport
