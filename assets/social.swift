// Generates the 1280x640 image for GitHub's repository social preview — the card
// that renders wherever a link to the repo is posted.
//
// GitHub has no API for this, so the result is uploaded once by hand through
// Settings -> General -> Social preview:
//
//     swift assets/social.swift assets/social-preview.png
//
// The icon is not redrawn here: this shells out to assets/icon.swift, so the card
// and the icon cannot drift apart the first time the icon changes.
import AppKit

let out = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "assets/social-preview.png"
let rgb = CGColorSpaceCreateDeviceRGB()

// Render the icon at the size the card uses, straight from its own generator.
let iconPath = NSTemporaryDirectory() + "coins-social-icon.png"
let gen = Process()
gen.executableURL = URL(fileURLWithPath: "/usr/bin/env")
gen.arguments = ["swift", "assets/icon.swift", iconPath, "300"]
gen.standardOutput = FileHandle.nullDevice
try gen.run(); gen.waitUntilExit()
guard gen.terminationStatus == 0, let icon = NSImage(contentsOfFile: iconPath) else {
    FileHandle.standardError.write("assets/icon.swift failed\n".data(using: .utf8)!)
    exit(1)
}

// GitHub renders at 1280x640 and trims the edges, so everything sits well inside.
let W = 1280, H = 640
// Opaque, with no alpha channel: the card is a solid rectangle, and sRGB explicitly
// so the colours do not shift once it is served on the web.
let ctx = CGContext(data: nil, width: W, height: H, bitsPerComponent: 8, bytesPerRow: 0,
                    space: CGColorSpace(name: CGColorSpace.sRGB)!,
                    bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue)!
ctx.setAllowsAntialiasing(true)
ctx.interpolationQuality = .high

// The terminal the tool lives in: theme.rs's dark ground, lit from the top.
ctx.drawLinearGradient(
    CGGradient(colorsSpace: rgb, colors: [
        CGColor(red: 0.129, green: 0.129, blue: 0.122, alpha: 1),
        CGColor(red: 0.043, green: 0.043, blue: 0.039, alpha: 1)] as CFArray,
        locations: [0, 1])!,
    start: CGPoint(x: 0, y: CGFloat(H)), end: .zero, options: [])

NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(cgContext: ctx, flipped: false)

let iconSide: CGFloat = 300
// A soft shadow, because the icon's tile is the same near-black as the card: without
// it the tile edge disappears and the chart looks like it floats in the background.
ctx.setShadow(offset: CGSize(width: 0, height: -10), blur: 34,
              color: CGColor(red: 0, green: 0, blue: 0, alpha: 0.75))
icon.draw(in: CGRect(x: 96, y: (CGFloat(H) - iconSide)/2, width: iconSide, height: iconSide))
ctx.setShadow(offset: .zero, blur: 0, color: nil)

// The name in the same monospace the output is read in, and one line of what it is.
func text(_ s: String, _ font: NSFont, _ color: NSColor, x: CGFloat, y: CGFloat) {
    s.draw(at: CGPoint(x: x, y: y), withAttributes: [.font: font, .foregroundColor: color])
}
let x: CGFloat = 470
text("coins", NSFont.monospacedSystemFont(ofSize: 108, weight: .semibold),
     NSColor(red: 0.96, green: 0.96, blue: 0.94, alpha: 1), x: x, y: 372)
// Wrapped by hand at a width that clears the edge GitHub trims.
let muted = NSColor(red: 0.65, green: 0.65, blue: 0.62, alpha: 1)
let body = NSFont.monospacedSystemFont(ofSize: 30, weight: .regular)
text("Cryptocurrency prices, charts", body, muted, x: x, y: 300)
text("and holdings in your terminal", body, muted, x: x, y: 254)
// A prompt, because the tool is one word you type.
text("$ coins", NSFont.monospacedSystemFont(ofSize: 30, weight: .medium),
     NSColor(red: 0.22, green: 0.53, blue: 0.90, alpha: 1), x: x, y: 168)

NSGraphicsContext.restoreGraphicsState()

let image = ctx.makeImage()!
let dest = CGImageDestinationCreateWithURL(URL(fileURLWithPath: out) as CFURL,
                                           "public.png" as CFString, 1, nil)!
CGImageDestinationAddImage(dest, image, nil)
guard CGImageDestinationFinalize(dest) else {
    FileHandle.standardError.write("could not write \(out)\n".data(using: .utf8)!)
    exit(1)
}
print("wrote \(out) at \(W)x\(H)")
