// Navigation replay on macOS with public CGEvents (CI runners; see
// .github/workflows/nav.yml). Posts notched wheel events, continuous
// (trackpad / Magic Mouse) scroll events with their scroll and momentum
// phases, and mouse drags to the noshape window, logging every posted event
// into the app's navigation trace ({"posted": …}) so check.py can recompute
// what Chrome hands Onshape for the very same CGEvents.
//
//   swift mac-replay.swift <trace.jsonl>
//
// Magnify (pinch) has no public way to synthesize it; the app's unit tests
// feed it at gpui's event boundary instead (navigation.rs
// `pinch_updates_zoom_one_notch_each`).

import CoreGraphics
import Foundation

let tracePath = CommandLine.arguments[1]

func readTrace() -> [[String: Any]] {
    guard let text = try? String(contentsOfFile: tracePath, encoding: .utf8) else { return [] }
    return text.split(separator: "\n").compactMap { try? JSONSerialization.jsonObject(with: Data($0.utf8)) as? [String: Any] }
}

func append(_ record: [String: Any]) {
    let data = try! JSONSerialization.data(withJSONObject: record)
    let handle = FileHandle(forWritingAtPath: tracePath)!
    handle.seekToEndOfFile()
    handle.write(data + Data("\n".utf8))
    handle.closeFile()
}

func pause(_ s: Double) { Thread.sleep(forTimeInterval: s) }

// ------------------------------------------------------------ where the viewport is

var origin = (0.0, 0.0), size = (0.0, 0.0)
let deadline = Date().addingTimeInterval(240)
while true {
    if let cam = readTrace().last(where: { $0["camera"] != nil }), let o = cam["origin"] as? [Double], let v = cam["viewport"] as? [Double] {
        origin = (o[0], o[1]); size = (v[0], v[1]); break
    }
    if Date() > deadline { print("noshape never traced a camera"); exit(1) }
    // Nudge the camera so the viewport traces it (a zero-sum wheel).
    if let w = window() {
        let p = CGPoint(x: w.midX, y: w.midY)
        post(CGEvent(mouseEventSource: nil, mouseType: .mouseMoved, mouseCursorPosition: p, mouseButton: .left))
        for n: Int32 in [1, -1] { post(CGEvent(scrollWheelEvent2Source: nil, units: .line, wheelCount: 1, wheel1: n, wheel2: 0, wheel3: 0)); pause(0.3) }
    }
    pause(2)
}
pause(8)

/// The noshape window's frame on screen (CGWindowList, top-left origin).
func window() -> CGRect? {
    let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
    for w in list where (w[kCGWindowOwnerName as String] as? String)?.lowercased().contains("noshape") == true {
        if let b = w[kCGWindowBounds as String] as? [String: Double], let width = b["Width"], width > 400 {
            return CGRect(x: b["X"]!, y: b["Y"]!, width: width, height: b["Height"]!)
        }
    }
    return nil
}

func post(_ e: CGEvent?) { e?.post(tap: .cghidEventTap) }

// Window content offset on screen: calibrated with a mouse move, whose
// viewport-local position the app traces.
var offset = (0.0, 0.0)
if let w = window() {
    offset = (w.minX, w.minY)
    let probe = CGPoint(x: w.minX + origin.0 + size.0 / 2, y: w.minY + origin.1 + size.1 / 2)
    post(CGEvent(mouseEventSource: nil, mouseType: .mouseMoved, mouseCursorPosition: probe, mouseButton: .left))
    pause(0.5)
    if let mv = readTrace().last(where: { ($0["input"] as? String) == "mouse_move" }), let d = mv["data"] as? [String: Any], let at = d["at"] as? [Double] {
        // probe = offset + origin + at
        offset = (Double(probe.x) - origin.0 - at[0], Double(probe.y) - origin.1 - at[1])
    }
}

func point(_ dx: Double = 0, _ dy: Double = 0) -> CGPoint {
    CGPoint(x: offset.0 + origin.0 + size.0 / 2 + dx, y: offset.1 + origin.1 + size.1 / 2 + dy)
}

func moveTo(_ p: CGPoint) {
    post(CGEvent(mouseEventSource: nil, mouseType: .mouseMoved, mouseCursorPosition: p, mouseButton: .left))
    pause(0.15)
}

func step(_ name: String, _ body: () -> Void) {
    append(["step": name, "at": "start"])
    body()
    pause(0.6)
    append(["step": name, "at": "end"])
}

/// A notched wheel event of `lines` detents (kCGScrollWheelEventDeltaAxis1).
func notch(_ lines: Int32, flags: CGEventFlags = []) {
    let e = CGEvent(scrollWheelEvent2Source: nil, units: .line, wheelCount: 1, wheel1: lines, wheel2: 0, wheel3: 0)!
    e.setIntegerValueField(.scrollWheelEventIsContinuous, value: 0)
    e.flags = flags
    append(["posted": ["kind": "scroll", "continuous": false, "shift": flags.contains(.maskShift),
                       "axis1": e.getIntegerValueField(.scrollWheelEventDeltaAxis1),
                       "axis2": e.getIntegerValueField(.scrollWheelEventDeltaAxis2)]])
    post(e)
    pause(0.12)
}

/// A continuous (trackpad) scroll event of `dy` points with its phases
/// (kCGScrollWheelEventScrollPhase 1 began, 2 changed, 4 ended; momentum
/// phase 1 begin, 2 continue, 3 end).
func pixels(_ dy: Int32, _ dx: Int32 = 0, phase: Int64, momentum: Int64 = 0) {
    let e = CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 2, wheel1: dy, wheel2: dx, wheel3: 0)!
    e.setIntegerValueField(.scrollWheelEventIsContinuous, value: 1)
    e.setIntegerValueField(.scrollWheelEventScrollPhase, value: phase)
    e.setIntegerValueField(.scrollWheelEventMomentumPhase, value: momentum)
    append(["posted": ["kind": "scroll", "continuous": true,
                       "axis1": e.getIntegerValueField(.scrollWheelEventPointDeltaAxis1),
                       "axis2": e.getIntegerValueField(.scrollWheelEventPointDeltaAxis2)]])
    post(e)
    pause(0.016)
}

func drag(_ button: CGMouseButton, _ dx: Double, _ dy: Double, still: Double) {
    let (down, dragged, up): (CGEventType, CGEventType, CGEventType) = button == .right
        ? (.rightMouseDown, .rightMouseDragged, .rightMouseUp)
        : (.otherMouseDown, .otherMouseDragged, .otherMouseUp)
    var p = point()
    moveTo(p)
    post(CGEvent(mouseEventSource: nil, mouseType: down, mouseCursorPosition: p, mouseButton: button))
    pause(0.05)
    for _ in 0..<20 {
        p = CGPoint(x: p.x + dx / 20, y: p.y + dy / 20)
        let e = CGEvent(mouseEventSource: nil, mouseType: dragged, mouseCursorPosition: p, mouseButton: button)!
        e.setIntegerValueField(.mouseEventDeltaX, value: Int64(dx / 20))
        e.setIntegerValueField(.mouseEventDeltaY, value: Int64(dy / 20))
        post(e)
        pause(0.016)
    }
    pause(still)
    post(CGEvent(mouseEventSource: nil, mouseType: up, mouseCursorPosition: p, mouseButton: button))
    pause(still < 0.05 ? 1.2 : 0.2)
}

// ------------------------------------------------------------ steps

step("wheel") {
    moveTo(point(150, 80))
    for n: Int32 in [1, 1, 1, -1, -1, -1, 3, -3] { notch(n) }
}
step("shift-wheel") {
    moveTo(point(-120, -60))
    for n: Int32 in [1, -1] { notch(n, flags: .maskShift) }
}
step("ctrl-wheel") {
    moveTo(point(-120, -60))
    for n: Int32 in [1, -1] { notch(n, flags: .maskControl) }
}
step("pad-scroll-up") {
    moveTo(point(100, -50))
    pixels(0, phase: 1)
    for d: Int32 in [3, 7, 12, 15, 12, 7, 3] { pixels(d, phase: 2) }
    pixels(0, phase: 4)
    for (i, d) in ([6, 4, 3, 2, 1, 1] as [Int32]).enumerated() { pixels(d, phase: 0, momentum: i == 0 ? 1 : 2) }
    pixels(0, phase: 0, momentum: 3)
}
step("pad-scroll-down") {
    moveTo(point(100, -50))
    pixels(0, phase: 1)
    for d: Int32 in [-3, -7, -12, -15, -12, -7, -3] { pixels(d, phase: 2) }
    pixels(0, phase: 4)
}
step("pad-scroll-sideways") {
    moveTo(point(100, -50))
    pixels(0, phase: 1)
    for d: Int32 in [5, 10, 10, 5] { pixels(1, d, phase: 2) }
    pixels(0, phase: 4)
}
step("right-drag") { drag(.right, -200, 0, still: 0.1) }
step("right-drag-fling") { drag(.right, -200, 0, still: 0.0) }
step("middle-drag") { drag(.center, 200, 100, still: 0.1) }
print("mac replay done")
