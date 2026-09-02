// Generates the repository icon: a braille-dot price chart on a dark tile.
//
// Run by hand and commit the result; nothing at build or run time needs Swift:
//     swift assets/icon.swift docs/icon.png 256
//
// The colours are theme.rs's dark palette, so the icon and the terminal agree:
// series blue for the line, the up-green for the leg that is rising, the axis grey
// for the frame.
import AppKit

let out = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "docs/icon.png"
let size = CommandLine.arguments.count > 2 ? Int(CommandLine.arguments[2])! : 256

let rgb   = CGColorSpaceCreateDeviceRGB()
let blue  = CGColor(red: 0x39/255.0, green: 0x87/255.0, blue: 0xe5/255.0, alpha: 1)
let green = CGColor(red: 0x0c/255.0, green: 0xa3/255.0, blue: 0x0c/255.0, alpha: 1)
let axis  = CGColor(red: 0x50/255.0, green: 0x50/255.0, blue: 0x4c/255.0, alpha: 1)
let tileT = CGColor(red: 0.129, green: 0.129, blue: 0.122, alpha: 1)
let tileB = CGColor(red: 0.055, green: 0.055, blue: 0.051, alpha: 1)

// The series, as fractions of the plot height: a month of a coin that dips, turns,
// and ends higher than it started. Dense enough that the dots read as a line.
let series: [Double] = [0.30, 0.25, 0.31, 0.27, 0.20, 0.24, 0.19, 0.23,
                        0.28, 0.26, 0.34, 0.41, 0.38, 0.47, 0.55, 0.52,
                        0.61, 0.70, 0.79, 0.88]
// Where the line turns and the up-colour takes over.
let turn = 10

func draw(_ ctx: CGContext, _ s: CGFloat) {
    // The tile: inset and heavily rounded, which is what makes it read as an icon
    // rather than as a picture of a chart.
    let inset = s * 0.055
    let tile = CGRect(x: inset, y: inset, width: s - 2*inset, height: s - 2*inset)
    ctx.saveGState()
    ctx.addPath(CGPath(roundedRect: tile, cornerWidth: s*0.224, cornerHeight: s*0.224,
                       transform: nil))
    ctx.clip()
    ctx.drawLinearGradient(
        CGGradient(colorsSpace: rgb, colors: [tileT, tileB] as CFArray, locations: [0, 1])!,
        start: CGPoint(x: 0, y: s), end: .zero, options: [])

    // The plot area, with the axis drawn as the tool draws it: a left rail and a
    // baseline, both recessive.
    let left = s * 0.245, right = s * 0.775
    let bottom = s * 0.285, top = s * 0.745
    ctx.setStrokeColor(axis)
    ctx.setLineWidth(s * 0.014)
    ctx.setLineCap(.round)
    ctx.move(to: CGPoint(x: left, y: top));    ctx.addLine(to: CGPoint(x: left, y: bottom))
    ctx.move(to: CGPoint(x: left, y: bottom)); ctx.addLine(to: CGPoint(x: right, y: bottom))
    ctx.strokePath()

    // The line itself, as discrete dots: two per column, the way a braille cell
    // inks the dot below to give the line weight.
    let dot = s * 0.042
    let colW = (right - left - dot) / CGFloat(series.count - 1)
    for (i, v) in series.enumerated() {
        let x = left + dot*0.9 + CGFloat(i) * colW
        let y = bottom + CGFloat(v) * (top - bottom)
        // The rising leg is the up-colour; green and red are direction, never decoration.
        ctx.setFillColor(i >= turn ? green : blue)
        for k in 0..<2 {
            let cy = y - CGFloat(k) * dot * 0.72
            ctx.fillEllipse(in: CGRect(x: x - dot/2, y: cy - dot/2, width: dot, height: dot))
        }
    }
    ctx.restoreGState()
}

let ctx = CGContext(data: nil, width: size, height: size, bitsPerComponent: 8,
                    bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!,
                    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
ctx.setAllowsAntialiasing(true)
draw(ctx, CGFloat(size))

let image = ctx.makeImage()!
let url = URL(fileURLWithPath: out)
let dest = CGImageDestinationCreateWithURL(url as CFURL, "public.png" as CFString, 1, nil)!
CGImageDestinationAddImage(dest, image, nil)
guard CGImageDestinationFinalize(dest) else {
    FileHandle.standardError.write("could not write \(out)\n".data(using: .utf8)!)
    exit(1)
}
print("wrote \(out) at \(size)x\(size)")
