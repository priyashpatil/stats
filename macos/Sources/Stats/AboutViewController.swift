import AppKit

@MainActor
final class AboutViewController: NSViewController {
  override func loadView() {
    preferredContentSize = NSSize(width: 520, height: 400)
    view = NSView(frame: NSRect(origin: .zero, size: preferredContentSize))

    let icon = NSImageView()
    icon.image = NSApp.applicationIconImage
    icon.imageScaling = .scaleProportionallyUpOrDown
    icon.translatesAutoresizingMaskIntoConstraints = false
    NSLayoutConstraint.activate([
      icon.widthAnchor.constraint(equalToConstant: 96),
      icon.heightAnchor.constraint(equalToConstant: 96),
    ])

    let name = NSTextField(labelWithString: "Stats")
    name.font = .systemFont(ofSize: 24, weight: .semibold)
    let version =
      Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String
      ?? "Development"
    let versionLabel = NSTextField(labelWithString: "Version \(version)")
    versionLabel.textColor = .secondaryLabelColor
    let description = NSTextField(
      wrappingLabelWithString: "A terminal dashboard for macOS system metrics and AI coding usage."
    )
    description.alignment = .center
    description.textColor = .secondaryLabelColor
    description.maximumNumberOfLines = 2
    description.preferredMaxLayoutWidth = 360
    description.widthAnchor.constraint(equalToConstant: 360).isActive = true

    let changelogButton = NSButton(
      title: "Changelog",
      target: self,
      action: #selector(openChangelog(_:))
    )
    changelogButton.bezelStyle = .rounded

    let stack = NSStackView(views: [icon, name, versionLabel, description, changelogButton])
    stack.orientation = .vertical
    stack.alignment = .centerX
    stack.spacing = 8
    stack.setCustomSpacing(14, after: versionLabel)
    stack.setCustomSpacing(16, after: description)
    stack.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(stack)
    NSLayoutConstraint.activate([
      stack.centerXAnchor.constraint(equalTo: view.centerXAnchor),
      stack.topAnchor.constraint(equalTo: view.topAnchor, constant: 48),
    ])
  }

  @objc private func openChangelog(_ sender: NSButton) {
    NSWorkspace.shared.open(URL(string: "https://github.com/priyashpatil/stats/releases")!)
  }
}
