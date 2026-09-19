// Renders the banner at the top of README.md.
//   swift design/readme-banner.swift design/readme
// Same dotted sphere as the recording orb and the installer window.

import AppKit

let width: CGFloat = 1280
let height: CGFloat = 560

func render(to path: String, icon: NSImage) {
  let rep = NSBitmapImageRep(
    bitmapDataPlanes: nil, pixelsWide: Int(width * 2), pixelsHigh: Int(height * 2),
    bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
    colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
  rep.size = NSSize(width: width, height: height)
  NSGraphicsContext.saveGraphicsState()
  let context = NSGraphicsContext(bitmapImageRep: rep)!
  NSGraphicsContext.current = context
  let cg = context.cgContext

  // Backdrop with a soft glow behind the sphere.
  let space = CGColorSpaceCreateDeviceRGB()
  cg.setFillColor(CGColor(red: 0.047, green: 0.047, blue: 0.051, alpha: 1))
  cg.fill(CGRect(x: 0, y: 0, width: width, height: height))
  let glow = CGGradient(
    colorsSpace: space, colors: [CGColor(gray: 1, alpha: 0.08), CGColor(gray: 1, alpha: 0)] as CFArray,
    locations: [0, 1])!
  let center = CGPoint(x: width / 2, y: height / 2)
  cg.drawRadialGradient(glow, startCenter: center, startRadius: 0, endCenter: center, endRadius: 420, options: [])

  drawSphere(cg, center: center, radius: 240)

  // App icon, then the name and tagline under it. AppKit's origin is bottom-left here.
  let iconSize: CGFloat = 128
  icon.draw(in: NSRect(x: center.x - iconSize / 2, y: center.y - 10, width: iconSize, height: iconSize))
  drawText("Parla", size: 56, weight: .bold, alpha: 0.97, baseline: center.y - 78, tracking: -1.2)
  drawText(
    "Hold a key, talk, and your words are typed. Entirely on your Mac.", size: 22, weight: .regular,
    alpha: 0.55, baseline: center.y - 122)

  NSGraphicsContext.restoreGraphicsState()
  try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: path))
}

func drawSphere(_ cg: CGContext, center: CGPoint, radius: CGFloat) {
  let yaw = 0.6
  let tilt = -0.38
  let rings = 24
  let density = 80.0
  var dots: [(x: CGFloat, y: CGFloat, z: Double)] = []
  for ring in 0...rings {
    let lat = -Double.pi / 2 + Double(ring) / Double(rings) * Double.pi
    let wave = 0.62 * sin(1.3 - Double(ring) * 0.52) + 0.38 * sin(0.9 + Double(ring) * 0.83)
    let r = 0.9 + 0.06 * wave
    let count = max(1, Int((abs(cos(lat)) * density).rounded()))
    for j in 0..<count {
      let lon = Double(j) / Double(count) * 2 * Double.pi
      let x = cos(lat) * cos(lon) * r
      let y = sin(lat) * r
      let z = cos(lat) * sin(lon) * r
      let x1 = x * cos(yaw) + z * sin(yaw)
      let z1 = -x * sin(yaw) + z * cos(yaw)
      let y1 = y * cos(tilt) - z1 * sin(tilt)
      let z2 = y * sin(tilt) + z1 * cos(tilt)
      dots.append((center.x + CGFloat(x1) * radius, center.y + CGFloat(y1) * radius, z2))
    }
  }
  for dot in dots.sorted(by: { $0.z < $1.z }) {
    let depth = (dot.z + 1) / 2
    let size = CGFloat(0.8 + 1.8 * depth)
    cg.setFillColor(CGColor(gray: 1, alpha: CGFloat(0.04 + 0.14 * depth)))
    cg.fillEllipse(in: CGRect(x: dot.x - size, y: dot.y - size, width: size * 2, height: size * 2))
  }
}

func drawText(
  _ string: String, size: CGFloat, weight: NSFont.Weight, alpha: CGFloat, baseline: CGFloat,
  tracking: CGFloat = 0
) {
  let style = NSMutableParagraphStyle()
  style.alignment = .center
  let text = NSAttributedString(
    string: string,
    attributes: [
      .font: NSFont.systemFont(ofSize: size, weight: weight),
      .foregroundColor: NSColor(white: 1, alpha: alpha),
      .paragraphStyle: style,
      .kern: tracking,
    ])
  text.draw(in: NSRect(x: 0, y: baseline, width: width, height: size * 1.4))
}

let folder = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "."
try? FileManager.default.createDirectory(atPath: folder, withIntermediateDirectories: true)
let icon = NSImage(contentsOfFile: "design/icon-source.png")!
render(to: "\(folder)/banner.png", icon: icon)
