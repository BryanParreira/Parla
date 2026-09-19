// swift-tools-version:5.9

import PackageDescription

let package = Package(
  name: "parla-swift",
  platforms: [.macOS("15.0")],
  products: [
    .library(name: "parla-swift", type: .static, targets: ["ParlaSwift"])
  ],
  dependencies: [
    .package(url: "https://github.com/Brendonovich/swift-rs", exact: "1.0.7"),
    .package(url: "https://github.com/soniqo/speech-swift", exact: "0.0.9"),
  ],
  targets: [
    .target(
      name: "ParlaSwift",
      dependencies: [
        .product(name: "AudioCommon", package: "speech-swift"),
        .product(name: "ParakeetASR", package: "speech-swift"),
        .product(name: "ParakeetStreamingASR", package: "speech-swift"),
        .product(name: "SwiftRs", package: "swift-rs"),
      ],
      path: "src"
    )
  ]
)
