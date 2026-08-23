import AppKit

@MainActor
final class SettingsWindowController: NSWindowController, NSTableViewDataSource,
  NSTableViewDelegate
{
  private let detailController = NSViewController()
  private var panes: [NSViewController] = []
  private let sidebarItems = [
    (title: "General", symbol: "gearshape"),
    (title: "Clocks", symbol: "clock"),
    (title: "About", symbol: "info.circle"),
  ]
  private let sidebarTable = NSTableView()
  private let clockMenuWidth: CGFloat = 360
  private var clockPickerPanel: ClockPickerPanel?
  private var selectedClockChoices: [ClockChoice]
  private let onLaunchAtLoginChange: (Bool) -> Bool
  private let onClockChoicesChange: ([ClockChoice]) -> Void

  init(
    selectedClockChoices: [ClockChoice],
    launchesAtLogin: Bool,
    onLaunchAtLoginChange: @escaping (Bool) -> Bool,
    onClockChoicesChange: @escaping ([ClockChoice]) -> Void
  ) {
    self.selectedClockChoices = selectedClockChoices
    self.onLaunchAtLoginChange = onLaunchAtLoginChange
    self.onClockChoicesChange = onClockChoicesChange
    let window = NSWindow(
      contentRect: NSRect(x: 0, y: 0, width: 720, height: 440),
      styleMask: [.titled, .closable, .resizable],
      backing: .buffered,
      defer: false
    )
    super.init(window: window)
    window.title = "Stats Settings"
    window.isReleasedWhenClosed = false
    window.center()
    configureContent(selectedClockChoices: selectedClockChoices, launchesAtLogin: launchesAtLogin)
  }

  required init?(coder: NSCoder) {
    fatalError("init(coder:) has not been implemented")
  }

  private func configureContent(selectedClockChoices: [ClockChoice], launchesAtLogin: Bool) {
    panes = [
      generalViewController(launchesAtLogin: launchesAtLogin),
      clocksViewController(selectedClockChoices: selectedClockChoices),
      AboutViewController(),
    ]

    let sidebarController = NSViewController()
    let sidebar = NSVisualEffectView()
    sidebar.material = .sidebar
    sidebar.blendingMode = .behindWindow
    sidebar.state = .active
    sidebarController.view = sidebar

    let column = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("SettingsSidebar"))
    sidebarTable.addTableColumn(column)
    sidebarTable.headerView = nil
    sidebarTable.rowHeight = 32
    sidebarTable.intercellSpacing = NSSize(width: 0, height: 2)
    sidebarTable.style = .sourceList
    sidebarTable.selectionHighlightStyle = .sourceList
    sidebarTable.dataSource = self
    sidebarTable.delegate = self
    sidebarTable.backgroundColor = .clear

    let appIcon = NSImageView()
    appIcon.image = NSApp.applicationIconImage
    appIcon.imageScaling = .scaleProportionallyUpOrDown
    let appName = NSTextField(labelWithString: "Stats")
    appName.font = .systemFont(ofSize: 15, weight: .semibold)
    let appHeader = NSStackView(views: [appIcon, appName])
    appHeader.orientation = .horizontal
    appHeader.alignment = .centerY
    appHeader.spacing = 8
    appHeader.translatesAutoresizingMaskIntoConstraints = false
    sidebar.addSubview(appHeader)

    let separator = NSBox()
    separator.boxType = .separator
    separator.translatesAutoresizingMaskIntoConstraints = false
    sidebar.addSubview(separator)

    let scrollView = NSScrollView()
    scrollView.drawsBackground = false
    scrollView.documentView = sidebarTable
    scrollView.translatesAutoresizingMaskIntoConstraints = false
    sidebar.addSubview(scrollView)
    NSLayoutConstraint.activate([
      appHeader.leadingAnchor.constraint(equalTo: sidebar.leadingAnchor, constant: 12),
      appHeader.topAnchor.constraint(equalTo: sidebar.topAnchor, constant: 16),
      appIcon.widthAnchor.constraint(equalToConstant: 28),
      appIcon.heightAnchor.constraint(equalToConstant: 28),
      separator.leadingAnchor.constraint(equalTo: sidebar.leadingAnchor, constant: 12),
      separator.trailingAnchor.constraint(equalTo: sidebar.trailingAnchor, constant: -12),
      separator.topAnchor.constraint(equalTo: appHeader.bottomAnchor, constant: 12),
      scrollView.leadingAnchor.constraint(equalTo: sidebar.leadingAnchor),
      scrollView.trailingAnchor.constraint(equalTo: sidebar.trailingAnchor),
      scrollView.topAnchor.constraint(equalTo: separator.bottomAnchor, constant: 8),
      scrollView.bottomAnchor.constraint(equalTo: sidebar.bottomAnchor, constant: -12),
    ])

    detailController.view = NSView()
    let splitController = NSSplitViewController()
    let sidebarItem = NSSplitViewItem(sidebarWithViewController: sidebarController)
    sidebarItem.canCollapse = false
    sidebarItem.minimumThickness = 180
    sidebarItem.maximumThickness = 220
    splitController.addSplitViewItem(sidebarItem)
    splitController.addSplitViewItem(NSSplitViewItem(viewController: detailController))
    window?.contentViewController = splitController
    window?.contentMinSize = NSSize(width: 620, height: 360)

    sidebarTable.selectRowIndexes(IndexSet(integer: 0), byExtendingSelection: false)
    showPane(at: 0)
  }

  private func generalViewController(launchesAtLogin: Bool) -> NSViewController {
    let controller = settingsPane()
    let title = NSTextField(labelWithString: "General")
    title.font = .systemFont(ofSize: 22, weight: .semibold)
    let checkbox = NSButton(
      checkboxWithTitle: "Launch Stats at login",
      target: self,
      action: #selector(launchAtLoginChanged(_:))
    )
    checkbox.state = launchesAtLogin ? .on : .off
    let help = NSTextField(
      wrappingLabelWithString: "Automatically open Stats when you sign in to your Mac."
    )
    help.textColor = .secondaryLabelColor

    let stack = NSStackView(views: [title, checkbox, help])
    stack.orientation = .vertical
    stack.alignment = .leading
    stack.spacing = 6
    stack.setCustomSpacing(24, after: title)
    install(stack, in: controller.view)
    return controller
  }

  private func clocksViewController(selectedClockChoices: [ClockChoice]) -> NSViewController {
    let controller = settingsPane()

    let title = NSTextField(labelWithString: "Clocks")
    title.font = .systemFont(ofSize: 22, weight: .semibold)
    let help = NSTextField(
      wrappingLabelWithString:
        "Type a city or time zone in any clock field, then choose a matching city."
    )
    help.textColor = .secondaryLabelColor

    var rows: [[NSView]] = []
    for slot in 0..<4 {
      let label = NSTextField(labelWithString: "Clock \(slot + 1):")
      label.alignment = .left
      let popup = ClockPickerButton(frame: .zero, pullsDown: false)
      popup.addItem(withTitle: selectedClockChoices[slot].title(at: Date()))
      popup.onOpen = { [weak self] popup in
        self?.showClockPicker(for: popup, slot: slot)
      }
      popup.cell?.lineBreakMode = .byTruncatingTail
      popup.widthAnchor.constraint(equalToConstant: clockMenuWidth).isActive = true
      rows.append([label, popup])
    }

    let grid = NSGridView(views: rows)
    grid.rowSpacing = 12
    grid.columnSpacing = 12
    grid.column(at: 0).xPlacement = .leading
    grid.column(at: 0).width = 72
    grid.column(at: 1).xPlacement = .fill
    grid.column(at: 1).width = 360

    let stack = NSStackView(views: [title, help, grid])
    stack.orientation = .vertical
    stack.alignment = .leading
    stack.spacing = 10
    stack.setCustomSpacing(8, after: title)
    stack.setCustomSpacing(20, after: help)
    install(stack, in: controller.view)
    return controller
  }

  private func settingsPane() -> NSViewController {
    let controller = NSViewController()
    controller.preferredContentSize = NSSize(width: 520, height: 400)
    controller.view = NSView(frame: NSRect(origin: .zero, size: controller.preferredContentSize))
    return controller
  }

  private func install(_ stack: NSStackView, in view: NSView) {
    stack.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(stack)
    NSLayoutConstraint.activate([
      stack.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 28),
      stack.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -28),
      stack.topAnchor.constraint(equalTo: view.topAnchor, constant: 28),
    ])
  }

  func numberOfRows(in tableView: NSTableView) -> Int {
    sidebarItems.count
  }

  func tableView(
    _ tableView: NSTableView,
    viewFor tableColumn: NSTableColumn?,
    row: Int
  ) -> NSView? {
    let item = sidebarItems[row]
    let cell = NSTableCellView()
    let image = NSImageView()
    image.image = NSImage(systemSymbolName: item.symbol, accessibilityDescription: item.title)
    image.symbolConfiguration = NSImage.SymbolConfiguration(pointSize: 14, weight: .regular)
    let label = NSTextField(labelWithString: item.title)
    let stack = NSStackView(views: [image, label])
    stack.orientation = .horizontal
    stack.alignment = .centerY
    stack.spacing = 8
    stack.translatesAutoresizingMaskIntoConstraints = false
    cell.addSubview(stack)
    NSLayoutConstraint.activate([
      stack.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 6),
      stack.trailingAnchor.constraint(lessThanOrEqualTo: cell.trailingAnchor, constant: -6),
      stack.centerYAnchor.constraint(equalTo: cell.centerYAnchor),
      image.widthAnchor.constraint(equalToConstant: 18),
    ])
    return cell
  }

  func tableViewSelectionDidChange(_ notification: Notification) {
    let row = sidebarTable.selectedRow
    if row >= 0 {
      showPane(at: row)
    }
  }

  private func showPane(at index: Int) {
    for child in detailController.children {
      child.view.removeFromSuperview()
      child.removeFromParent()
    }
    let pane = panes[index]
    detailController.addChild(pane)
    pane.view.translatesAutoresizingMaskIntoConstraints = false
    detailController.view.addSubview(pane.view)
    NSLayoutConstraint.activate([
      pane.view.leadingAnchor.constraint(equalTo: detailController.view.leadingAnchor),
      pane.view.trailingAnchor.constraint(equalTo: detailController.view.trailingAnchor),
      pane.view.topAnchor.constraint(equalTo: detailController.view.topAnchor),
      pane.view.bottomAnchor.constraint(equalTo: detailController.view.bottomAnchor),
    ])
  }

  @objc private func launchAtLoginChanged(_ sender: NSButton) {
    let requested = sender.state == .on
    sender.state = onLaunchAtLoginChange(requested) ? .on : .off
  }

  private func showClockPicker(for popup: ClockPickerButton, slot: Int) {
    let picker = ClockPickerViewController { [weak self, weak popup] choice in
      guard let self, let popup else { return }
      self.selectedClockChoices[slot] = choice
      popup.item(at: 0)?.title = choice.title(at: Date())
      self.clockPickerPanel?.close()
      self.onClockChoicesChange(self.selectedClockChoices)
    }
    guard let window = popup.window else { return }
    let anchor = window.convertToScreen(popup.convert(popup.bounds, to: nil))
    let panel = ClockPickerPanel(
      contentRect: NSRect(
        x: anchor.minX,
        y: anchor.minY - 320,
        width: clockMenuWidth,
        height: 320
      ),
      styleMask: .borderless,
      backing: .buffered,
      defer: false
    )
    panel.isReleasedWhenClosed = false
    panel.hasShadow = true
    panel.backgroundColor = .windowBackgroundColor
    panel.contentViewController = picker
    panel.contentView?.wantsLayer = true
    panel.contentView?.layer?.cornerRadius = 8
    panel.contentView?.layer?.masksToBounds = true
    clockPickerPanel?.close()
    clockPickerPanel = panel
    panel.makeKeyAndOrderFront(nil)
  }
}
