import AppKit

// Offscreen design artifact only; does not open or control an application window.
let output = CommandLine.arguments[1]
let width = 1440
let height = 244
let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: width, pixelsHigh: height,
    bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
    colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
let context = NSGraphicsContext(bitmapImageRep: bitmap)!
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = context
func color(_ r: CGFloat, _ g: CGFloat, _ b: CGFloat) -> NSColor {
    NSColor(red: r / 255, green: g / 255, blue: b / 255, alpha: 1)
}
color(20, 27, 34).setFill()
NSRect(x: 0, y: 0, width: width, height: height).fill()
color(24, 42, 50).setFill()
NSRect(x: 0, y: 0, width: CGFloat(width) * 4 / 24, height: CGFloat(height)).fill()
func text(_ value: String, _ x: CGFloat, _ y: CGFloat, _ size: CGFloat,
          _ ink: NSColor, bold: Bool = false) {
    let font = NSFont(name: bold ? "AppleSDGothicNeo-Bold" : "AppleSDGothicNeo-Regular", size: size)!
    (value as NSString).draw(at: NSPoint(x: x, y: y), withAttributes: [.font: font, .foregroundColor: ink])
}
let main = color(238, 244, 245)
let secondary = color(168, 182, 194)
text("agent-progress 제품 완성", 26, 187, 21, main, bold: true)
text("16%  ·  4 / 24 항목 완료", 325, 188, 18, secondary)
text("현재 계획", 26, 145, 17, secondary)
text("Codex 기준 agent-progress 구현과 실행 검증", 130, 145, 18, main)
text("Ⅱ  일시 중지", 26, 103, 20, color(231, 182, 109), bold: true)
text("마지막 완료", 26, 62, 17, secondary)
text("Codex 종단 흐름·현재 버전 연동 검증", 130, 62, 18, secondary)
text("계획  docs/product-completion-plan.md     ·     목표  ap.project.json", 26, 23, 15, color(125, 145, 159))
NSGraphicsContext.restoreGraphicsState()
try bitmap.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: output))
