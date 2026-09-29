// swift-tools-version: 6.2

import PackageDescription

let package = Package(
    name: "VideoCreaterAVFoundationExport",
    platforms: [.macOS(.v14)],
    products: [
        .executable(
            name: "video-creater-avfoundation-exporter",
            targets: ["VideoCreaterAVFoundationExport"]
        )
    ],
    targets: [
        .executableTarget(
            name: "VideoCreaterAVFoundationExport",
            path: "Sources"
        ),
        .testTarget(
            name: "VideoCreaterAVFoundationExportTests",
            dependencies: ["VideoCreaterAVFoundationExport"]
        )
    ]
)
