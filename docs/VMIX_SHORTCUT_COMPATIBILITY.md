# vMix-compatible shortcut names (restricted adapter)

OPENVIDEOMIX follows vMix's production workflow. The core now includes a **subset translator** for vMix shortcut function names. The translator does not expose an HTTP/TCP compatibility server; its output is a validated `ProductionCommand`, intended for future routing by a separate authenticated protocol adapter.

## Implemented translation

| vMix shortcut function | Input | OPENVIDEOMIX action |
| --- | --- | --- |
| Cut | None | CUT between Preview and Program |
| PreviewInput | 1-based rack position | Set Preview |
| OverlayInput1In | 1-based rack position | Set Overlay 1 **ON** (idempotent) |
| OverlayInput1Off | None | Set Overlay 1 **OFF** (idempotent) |
| AudioOn | 1-based rack position | Unmute input |
| AudioOff | 1-based rack position | Mute input |

vMix input positions are **not** the same as OPENVIDEOMIX's persistent `InputId` values. The translator resolves the number against the current input-rack ordering. Unsupported functions or out-of-range indices are rejected instead of pretending to succeed.

**Not supported yet:** Fade, Merge, Wipe, transitions with durations, Stingers, four overlay channels, recording/streaming triggers, vMix XML status format, TCP TALLY/ACTS subscriptions, external LAN connectivity, and vMix script execution. The current mixer does not have a real composited output and no such function should be advertised as ready.

## Next implementation stage

1. Expose a versioned authenticated adapter that may accept a carefully allowlisted subset of vMix-compatible commands; avoid reusing vMix's default unauthenticated port 8088 or TCP control port on production machines.
2. Add automated tests for invalid input indices, permission failure, duplicate commands and state feedback without hardware.
3. After adding actual 4-layer compositor and audio, extend function mapping and provide accurate tally updates.
4. Test against real OpenDeck/OpenAction/vMix-oriented control integrations. Never expose arbitrary scripting or uncontrolled browser GET commands without security review.

Official references:
- https://www.vmix.com/help29/DeveloperAPI.html
- https://www.vmix.com/help29/TCPAPI.html
- https://www.vmix.com/help29/ShortcutFunctionReference.html

Related: #13, #14, PR #16, PR #18.
