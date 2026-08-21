// swift-tools-version: 6.1

import PackageDescription

let package = Package(
  name: "Stats",
  platforms: [
    .macOS(.v11)
  ],
  dependencies: [
    .package(url: "https://github.com/migueldeicaza/SwiftTerm.git", from: "1.20.0")
  ],
  targets: [
    .executableTarget(
      name: "Stats",
      dependencies: ["SwiftTerm"]
    )
  ]
)
