// Renders the app icon: a honeycomb with a drip on a warm rounded square.
// Original vector artwork, no third-party assets.
// Usage: swift make-icon.swift <out.png>   (1024x1024 points)
import AppKit

let size: CGFloat = 1024
let out = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "icon.png"

let rep = NSBitmapImageRep(
    bitmapDataPlanes: nil, pixelsWide: Int(size), pixelsHigh: Int(size),
    bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
    colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)

func color(_ r: CGFloat, _ g: CGFloat, _ b: CGFloat, _ a: CGFloat = 1) -> NSColor {
    NSColor(calibratedRed: r / 255, green: g / 255, blue: b / 255, alpha: a)
}

// Background: macOS-style rounded square on the 1024 grid (824pt body).
let inset: CGFloat = 100
let body = NSRect(x: inset, y: inset, width: size - 2 * inset, height: size - 2 * inset)
let bg = NSBezierPath(roundedRect: body, xRadius: 185, yRadius: 185)
NSGradient(colors: [color(255, 214, 102), color(240, 150, 30)])!.draw(in: bg, angle: -90)

func hexagon(center: NSPoint, radius r: CGFloat) -> NSBezierPath {
    let p = NSBezierPath()
    for i in 0..<6 {
        let angle = CGFloat(i) * .pi / 3 + .pi / 6 // pointy top
        let pt = NSPoint(x: center.x + r * cos(angle), y: center.y + r * sin(angle))
        i == 0 ? p.move(to: pt) : p.line(to: pt)
    }
    p.close()
    return p
}

// Honeycomb: centre cell plus six neighbours.
let r: CGFloat = 118
let gap: CGFloat = 14
let step = (r * sqrt(3)) + gap
let centre = NSPoint(x: size / 2, y: size / 2 + 40)
var cells = [centre]
for i in 0..<6 {
    let a = CGFloat(i) * .pi / 3
    cells.append(NSPoint(x: centre.x + step * cos(a), y: centre.y + step * sin(a)))
}

let shadow = NSShadow()
shadow.shadowColor = color(120, 60, 0, 0.35)
shadow.shadowBlurRadius = 18
shadow.shadowOffset = NSSize(width: 0, height: -8)

for (i, c) in cells.enumerated() {
    NSGraphicsContext.saveGraphicsState()
    shadow.set()
    let outer = hexagon(center: c, radius: r)
    (i == 0 ? color(255, 248, 225) : color(255, 236, 170)).setFill()
    outer.fill()
    NSGraphicsContext.restoreGraphicsState()
    // Honey filling: full in the centre and lower cells, empty in the top cells.
    let filled = i == 0 || c.y < centre.y - 1
    if filled {
        let inner = hexagon(center: c, radius: r - 26)
        NSGradient(colors: [color(255, 196, 40), color(214, 120, 0)])!.draw(in: inner, angle: -90)
        // Glossy highlight.
        color(255, 255, 255, 0.35).setFill()
        NSBezierPath(ovalIn: NSRect(x: c.x - 42, y: c.y + 22, width: 52, height: 30)).fill()
    }
}

// Drip hanging from the lower-right cell's bottom corner.
let dripCell = cells[6] // angle 300°
let dripTop = NSPoint(x: dripCell.x, y: dripCell.y - (r - 26) + 10)
let drip = NSBezierPath()
drip.move(to: NSPoint(x: dripTop.x - 30, y: dripTop.y + 30))
drip.curve(to: NSPoint(x: dripTop.x, y: dripTop.y - 110),
           controlPoint1: NSPoint(x: dripTop.x - 28, y: dripTop.y - 30),
           controlPoint2: NSPoint(x: dripTop.x - 40, y: dripTop.y - 110))
drip.curve(to: NSPoint(x: dripTop.x + 30, y: dripTop.y + 30),
           controlPoint1: NSPoint(x: dripTop.x + 40, y: dripTop.y - 110),
           controlPoint2: NSPoint(x: dripTop.x + 28, y: dripTop.y - 30))
drip.close()
NSGradient(colors: [color(240, 160, 10), color(200, 105, 0)])!.draw(in: drip, angle: -90)

NSGraphicsContext.restoreGraphicsState()
let png = rep.representation(using: .png, properties: [:])!
try! png.write(to: URL(fileURLWithPath: out))
