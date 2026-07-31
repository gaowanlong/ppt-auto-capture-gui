# macOS Apple Silicon DMG Release Design

## Goal

Publish a macOS release artifact that Apple Silicon users can install through Finder as a normal `.app`, delivered inside a `.dmg`, instead of distributing a raw Mach-O executable mislabeled as an x86_64 build.

## Scope

This change affects only macOS release packaging and its documentation. Windows and Linux release artifacts remain unchanged.

The macOS artifact supports Apple Silicon (`arm64`) only. Universal and Intel builds are intentionally out of scope.

## Constraints

- No Apple Developer ID certificate is available.
- No notarization credentials are available.
- The application bundle must therefore use ad-hoc signing.
- Gatekeeper may require the user to right-click the application and choose **Open** on first launch.
- The workflow must not imply that the artifact is notarized or warning-free.

## Release Artifact

The GitHub Release macOS asset will be:

`ppt-auto-capture-gui-macos-apple-silicon.dmg`

The mounted disk image will contain:

- `PPT Auto Capture.app`
- An `Applications` symbolic link targeting `/Applications`

The application bundle will have this structure:

```text
PPT Auto Capture.app/
└── Contents/
    ├── Info.plist
    ├── MacOS/
    │   └── ppt-auto-capture-gui
    └── Resources/
```

An application icon is not required for this iteration because the repository does not currently contain an `.icns` asset. macOS will use the default application icon.

## Build Architecture

GitHub Actions will install and build the explicit Rust target:

`aarch64-apple-darwin`

The build must use:

```bash
cargo build --release --target aarch64-apple-darwin
```

The workflow must verify that the executable reports `arm64` through the macOS `file` command before packaging it.

## Bundle Metadata

`Info.plist` will define:

- `CFBundleName`: `PPT Auto Capture`
- `CFBundleDisplayName`: `PPT Auto Capture`
- `CFBundleExecutable`: `ppt-auto-capture-gui`
- `CFBundleIdentifier`: `com.gaowanlong.ppt-auto-capture`
- `CFBundlePackageType`: `APPL`
- `CFBundleShortVersionString`: derived from the release tag with the leading `v` removed
- `CFBundleVersion`: the same release version
- `LSMinimumSystemVersion`: `11.0`
- `LSArchitecturePriority`: `arm64`
- `NSHighResolutionCapable`: `true`

The version passed to the packaging script must be validated as a numeric dot-separated version such as `1.23`. Invalid input must stop the workflow.

## Signing

The complete `.app` bundle will be signed after assembly with:

```bash
codesign --force --deep --sign - "PPT Auto Capture.app"
```

The workflow will verify the result with:

```bash
codesign --verify --deep --strict --verbose=2 "PPT Auto Capture.app"
```

Ad-hoc signing establishes bundle integrity but does not provide developer identity or notarization.

## DMG Packaging

A repository script will own bundle and DMG construction so the behavior can be tested independently of GitHub Actions.

The script will:

1. Validate the release version and executable.
2. Verify the executable architecture is `arm64`.
3. Assemble the `.app` directory.
4. Generate `Info.plist`.
5. Apply and verify ad-hoc signing.
6. Create a staging directory containing the application and `/Applications` symlink.
7. Build a compressed read-only DMG with `hdiutil create`.
8. Mount the DMG into a temporary directory.
9. Verify the mounted application, executable, metadata, symlink, architecture, and signature.
10. Detach the DMG even when validation fails.

Temporary directories will be created with `mktemp -d` and cleaned through a shell trap.

The DMG does not need a custom background, icon positioning script, or license window. These are cosmetic and would add fragile Finder automation without improving application compatibility.

## Workflow Changes

The macOS job will:

- Be renamed to `Build macOS (Apple Silicon)`.
- Install `aarch64-apple-darwin`.
- Build the explicit target.
- Run the packaging script.
- Upload only the generated DMG and README.

The release job will:

- Download the renamed macOS artifact.
- Copy the DMG into `release-files`.
- Publish the DMG instead of the old x86_64 tarball.

Windows and Linux jobs and release files remain unchanged.

## Testing

Automated tests will cover observable packaging behavior:

- Reject malformed version strings.
- Reject a missing executable.
- Reject a non-arm64 executable.
- Produce a readable application bundle with the required `Info.plist` keys.
- Produce a valid ad-hoc signature.
- Produce a mountable DMG.
- Include an application bundle and a valid `/Applications` symlink in the mounted image.

The tests will use the locally built Apple Silicon executable on an Apple Silicon macOS host. GitHub Actions provides the authoritative end-to-end environment.

Static workflow checks will confirm that:

- The macOS target is explicitly `aarch64-apple-darwin`.
- The old `macos-x86_64` artifact name is absent.
- The release publishes the `.dmg`.

Before release, the full Rust test suite must continue to pass.

## Documentation

README installation instructions will explain:

1. Download and open the Apple Silicon DMG.
2. Drag `PPT Auto Capture.app` into `Applications`.
3. On first launch, right-click the app and select **Open** if Gatekeeper blocks it.
4. Grant Screen Recording permission when macOS requests it.
5. Restart the application after granting Screen Recording permission if necessary.

The documentation will explicitly state that the build supports Apple Silicon only and is not notarized.

## Success Criteria

- The GitHub Release contains `ppt-auto-capture-gui-macos-apple-silicon.dmg`.
- The DMG mounts successfully on an Apple Silicon Mac.
- The application can be copied to `/Applications` and launched through Finder.
- The executable inside the bundle is `arm64`.
- The application bundle passes strict ad-hoc signature verification.
- The release no longer publishes a macOS artifact labeled x86_64.
- Windows and Linux release artifacts continue to build and publish.
