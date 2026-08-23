import AppKit

@MainActor
final class ClockPickerButton: NSPopUpButton {
  var onOpen: ((ClockPickerButton) -> Void)?

  override func mouseDown(with event: NSEvent) {
    onOpen?(self)
  }
}

@MainActor
final class ClockPickerViewController: NSViewController, NSTableViewDataSource,
  NSTableViewDelegate, NSSearchFieldDelegate
{
  private let choices = ClockChoice.all
  private var filteredChoices = ClockChoice.all
  private let onSelect: (ClockChoice) -> Void
  private let searchField = NSSearchField()
  private let tableView = NSTableView()
  private let titles = ClockChoice.all.map { $0.title(at: Date()) }

  init(onSelect: @escaping (ClockChoice) -> Void) {
    self.onSelect = onSelect
    super.init(nibName: nil, bundle: nil)
  }

  required init?(coder: NSCoder) {
    fatalError("init(coder:) has not been implemented")
  }

  override func loadView() {
    view = NSView(frame: NSRect(x: 0, y: 0, width: 360, height: 320))

    searchField.placeholderString = "Search cities or time zones"
    searchField.delegate = self
    searchField.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(searchField)

    let column = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("ClockCity"))
    column.width = 344
    tableView.addTableColumn(column)
    tableView.headerView = nil
    tableView.rowHeight = 26
    tableView.intercellSpacing = NSSize(width: 0, height: 0)
    tableView.dataSource = self
    tableView.delegate = self
    tableView.target = self
    tableView.action = #selector(selectTableRow(_:))

    let scrollView = NSScrollView()
    scrollView.documentView = tableView
    scrollView.hasVerticalScroller = true
    scrollView.drawsBackground = false
    scrollView.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(scrollView)

    NSLayoutConstraint.activate([
      searchField.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 8),
      searchField.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -8),
      searchField.topAnchor.constraint(equalTo: view.topAnchor, constant: 8),
      scrollView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
      scrollView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
      scrollView.topAnchor.constraint(equalTo: searchField.bottomAnchor, constant: 6),
      scrollView.bottomAnchor.constraint(equalTo: view.bottomAnchor),
    ])
  }

  override func viewDidAppear() {
    super.viewDidAppear()
    view.window?.makeFirstResponder(searchField)
  }

  func numberOfRows(in tableView: NSTableView) -> Int {
    filteredChoices.count
  }

  func tableView(
    _ tableView: NSTableView,
    viewFor tableColumn: NSTableColumn?,
    row: Int
  ) -> NSView? {
    let choice = filteredChoices[row]
    let label = NSTextField(labelWithString: title(for: choice))
    label.lineBreakMode = .byTruncatingTail
    label.translatesAutoresizingMaskIntoConstraints = false
    let cell = NSTableCellView()
    cell.addSubview(label)
    NSLayoutConstraint.activate([
      label.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 8),
      label.trailingAnchor.constraint(equalTo: cell.trailingAnchor, constant: -8),
      label.centerYAnchor.constraint(equalTo: cell.centerYAnchor),
    ])
    return cell
  }

  func controlTextDidChange(_ notification: Notification) {
    let query = normalized(searchField.stringValue)
    filteredChoices = choices.filter {
      query.isEmpty || normalized($0.label).contains(query)
        || normalized($0.timezone).contains(query)
    }
    tableView.reloadData()
  }

  func control(
    _ control: NSControl,
    textView: NSTextView,
    doCommandBy commandSelector: Selector
  ) -> Bool {
    if commandSelector == #selector(NSResponder.moveDown(_:)) {
      moveSelection(by: 1)
      return true
    }
    if commandSelector == #selector(NSResponder.moveUp(_:)) {
      moveSelection(by: -1)
      return true
    }
    if commandSelector == #selector(NSResponder.insertNewline(_:))
      || commandSelector == #selector(NSResponder.insertNewlineIgnoringFieldEditor(_:))
    {
      commitSelectedChoice()
      return true
    }
    if commandSelector == #selector(NSResponder.cancelOperation(_:)) {
      view.window?.close()
      return true
    }
    return false
  }

  @objc private func selectTableRow(_ sender: NSTableView) {
    commitSelectedChoice()
  }

  private func commitSelectedChoice() {
    let row = tableView.selectedRow >= 0 ? tableView.selectedRow : 0
    guard filteredChoices.indices.contains(row) else { return }
    onSelect(filteredChoices[row])
  }

  private func moveSelection(by offset: Int) {
    guard !filteredChoices.isEmpty else { return }
    let currentRow = tableView.selectedRow
    let nextRow =
      currentRow < 0
      ? (offset > 0 ? 0 : filteredChoices.count - 1)
      : min(max(currentRow + offset, 0), filteredChoices.count - 1)
    tableView.selectRowIndexes(IndexSet(integer: nextRow), byExtendingSelection: false)
    tableView.scrollRowToVisible(nextRow)
  }

  private func title(for choice: ClockChoice) -> String {
    guard let index = choices.firstIndex(where: { $0.id == choice.id }) else { return choice.label }
    return titles[index]
  }

  private func normalized(_ value: String) -> String {
    value.folding(
      options: [.caseInsensitive, .diacriticInsensitive],
      locale: Locale.current
    )
  }
}

@MainActor
final class ClockPickerPanel: NSPanel {
  override var canBecomeKey: Bool { true }

  override func resignKey() {
    super.resignKey()
    close()
  }
}
