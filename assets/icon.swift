// Generates the project icon: the coin's rim, with a price line inside it.
//
// Run by hand and commit the result; nothing at build or run time needs Swift.
//     swift assets/icon.swift docs/icon.png 256
//
// Drawn on transparency in one colour, so it sits on a light or a dark page
// without a tile behind it. The gold is theme.rs's series yellow.
import AppKit

let out  = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "docs/icon.png"
let side = CommandLine.arguments.count > 2 ? Int(CommandLine.arguments[2])! : 256
let gold = CGColor(red: 0xc9/255.0, green: 0x85/255.0, blue: 0x00/255.0, alpha: 1)

func draw(_ ctx: CGContext, _ s: CGFloat) {
    ctx.setStrokeColor(gold)
    ctx.setLineCap(.round)
    ctx.setLineJoin(.round)

    // The rim. Slightly heavier than it looks like it needs, so the ring still
    // reads as a circle at favicon size rather than breaking into arcs.
    let d = s * 0.84
    ctx.setLineWidth(s * 0.082)
    ctx.strokeEllipse(in: CGRect(x: (s - d)/2, y: (s - d)/2, width: d, height: d))

    // The line: four points, angular, the shape the plot view draws — a dip, a
    // recovery, and a rise out of it.
    ctx.setLineWidth(s * 0.092)
    ctx.saveGState()
    ctx.scaleBy(x: 0.72, y: 0.72)
    ctx.translateBy(x: s * 0.195, y: s * 0.195)
    ctx.move(to: CGPoint(x: s*0.24, y: s*0.38))
    ctx.addLine(to: CGPoint(x: s*0.42, y: s*0.47))
    ctx.addLine(to: CGPoint(x: s*0.55, y: s*0.40))
    ctx.addLine(to: CGPoint(x: s*0.76, y: s*0.68))
    ctx.strokePath()
    ctx.restoreGState()
}

let ctx = CGContext(data: nil, width: side, height: side, bitsPerComponent: 8,
                    bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!,
                    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
ctx.setAllowsAntialiasing(true)
draw(ctx, CGFloat(side))

let dest = CGImageDestinationCreateWithURL(URL(fileURLWithPath: out) as CFURL,
                                           "public.png" as CFString, 1, nil)!
CGImageDestinationAddImage(dest, ctx.makeImage()!, nil)
guard CGImageDestinationFinalize(dest) else {
    FileHandle.standardError.write("could not write \(out)\n".data(using: .utf8)!)
    exit(1)
}
print("wrote \(out) at \(side)x\(side)")
