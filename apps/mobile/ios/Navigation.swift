import UIKit

@_silgen_name("datalith_open_document")
private func openDocument(_ path: UnsafePointer<CChar>)

@MainActor
private final class VaultController: UITableViewController, UISearchResultsUpdating {
    let folder: URL
    let document: UIViewController
    private let search = UISearchController(searchResultsController: nil)
    private var entries: [URL] = []

    init(folder: URL, document: UIViewController) {
        self.folder = folder
        self.document = document
        super.init(style: .insetGrouped)
        title = folder.lastPathComponent
    }
    required init?(coder: NSCoder) { nil }

    override func viewDidLoad() {
        super.viewDidLoad()
        tableView.register(UITableViewCell.self, forCellReuseIdentifier: "file")
        search.searchResultsUpdater = self
        search.obscuresBackgroundDuringPresentation = false
        navigationItem.searchController = search
        NotificationCenter.default.addObserver(self, selector: #selector(reloadFiles),
            name: .vaultChanged, object: nil)
    }

    @objc private func reloadFiles() { updateSearchResults(for: search) }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        updateSearchResults(for: search)
    }

    func updateSearchResults(for controller: UISearchController) {
        let query = controller.searchBar.text ?? ""
        do {
            entries = try FileManager.default.contentsOfDirectory(at: folder,
                includingPropertiesForKeys: [.isDirectoryKey], options: .skipsHiddenFiles)
                .filter { query.isEmpty || $0.lastPathComponent.localizedCaseInsensitiveContains(query) }
                .sorted { $0.lastPathComponent.localizedStandardCompare($1.lastPathComponent) == .orderedAscending }
            tableView.backgroundView = nil
        } catch {
            entries = []
            let message = UILabel()
            message.text = error.localizedDescription
            message.numberOfLines = 0
            tableView.backgroundView = message
        }
        tableView.reloadData()
    }

    override func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int { entries.count }

    override func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
        let url = entries[indexPath.row]
        let cell = tableView.dequeueReusableCell(withIdentifier: "file", for: indexPath)
        var content = cell.defaultContentConfiguration()
        content.text = url.lastPathComponent
        content.image = UIImage(systemName: url.hasDirectoryPath ? "folder" : "doc")
        cell.contentConfiguration = content
        cell.accessoryType = .disclosureIndicator
        return cell
    }

    override func tableView(_ tableView: UITableView, didSelectRowAt indexPath: IndexPath) {
        let url = entries[indexPath.row]
        if url.hasDirectoryPath {
            navigationController?.pushViewController(VaultController(folder: url, document: document), animated: true)
        } else {
            guard let nav = splitViewController?.viewController(for: .secondary) as? UINavigationController else { return }
            nav.setViewControllers([DocumentPage(url: url, surface: document)], animated: false)
            splitViewController?.show(.secondary)
        }
    }
}

@_cdecl("datalith_install_navigation")
@MainActor
func installNavigation(_ pointer: UnsafeMutableRawPointer, _ path: UnsafePointer<CChar>) {
    let view = Unmanaged<UIView>.fromOpaque(pointer).takeUnretainedValue()
    let folder = URL(fileURLWithPath: String(cString: path), isDirectory: true)
    DispatchQueue.main.async {
        guard let window = view.window, let document = window.rootViewController else { return }
        let split = UISplitViewController(style: .doubleColumn)
        let vault = VaultController(folder: folder, document: document)
        window.rootViewController = nil
        split.preferredDisplayMode = .oneBesideSecondary
        split.setViewController(UINavigationController(rootViewController: vault), for: .primary)
        let initial = DocumentPage(url: folder.appendingPathComponent("Welcome.md"), surface: document)
        split.setViewController(UINavigationController(rootViewController: initial), for: .secondary)
        window.rootViewController = split
        split.show(.primary)
    }
}

@_silgen_name("datalith_document_action")
private func runDocumentAction(_ delete: Bool)

@MainActor
private final class DocumentPage: UIViewController {
    static weak var visible: DocumentPage?
    let url: URL
    let surface: UIViewController
    var controls: [Int] = []

    init(url: URL, surface: UIViewController) {
        self.url = url
        self.surface = surface
        super.init(nibName: nil, bundle: nil)
        title = url.lastPathComponent
    }
    required init?(coder: NSCoder) { nil }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        Self.visible = self
        if surface.parent !== self {
            surface.willMove(toParent: nil)
            surface.view.removeFromSuperview()
            surface.removeFromParent()
            addChild(surface)
            view.addSubview(surface.view)
            surface.view.translatesAutoresizingMaskIntoConstraints = false
            let safe = view.safeAreaLayoutGuide
            NSLayoutConstraint.activate([
                surface.view.leadingAnchor.constraint(equalTo: safe.leadingAnchor),
                surface.view.trailingAnchor.constraint(equalTo: safe.trailingAnchor),
                surface.view.topAnchor.constraint(equalTo: safe.topAnchor),
                surface.view.bottomAnchor.constraint(equalTo: safe.bottomAnchor)
            ])
            surface.didMove(toParent: self)
        }
        url.path.withCString { openDocument($0) }
    }
}

@_cdecl("datalith_push_link")
@MainActor
func pushLink(_ path: UnsafePointer<CChar>) {
    let url = URL(fileURLWithPath: String(cString: path))
    DispatchQueue.main.async {
        guard let current = DocumentPage.visible, current.url != url else { return }
        current.navigationController?.pushViewController(DocumentPage(url: url, surface: current.surface), animated: true)
    }
}

@_cdecl("datalith_update_controls")
@MainActor
func updateControls(_ mode: Int32, _ writable: Bool) {
    guard let page = DocumentPage.visible else { return }
    let state = [Int(mode), writable ? 1 : 0]
    guard state != page.controls else { return }
    page.controls = state
    DispatchQueue.main.async {
        page.title = mode == 0 && !writable && !FileManager.default.fileExists(atPath: page.url.path) ? "File deleted" : page.url.lastPathComponent
        var items: [UIBarButtonItem] = []
        if mode != 0 {
            let action = UIAction(title: mode == 2 ? "Read" : "Edit",
                                  image: UIImage(systemName: mode == 2 ? "book" : "pencil")) { _ in
                runDocumentAction(false)
            }
            items.append(UIBarButtonItem(primaryAction: action))
        }
        if writable {
            let delete = UIAction(title: "Delete", image: UIImage(systemName: "trash"),
                                  attributes: .destructive) { _ in runDocumentAction(true) }
            let more = UIBarButtonItem(image: UIImage(systemName: "ellipsis.circle"), menu: UIMenu(children: [delete]))
            more.accessibilityLabel = "More actions"
            items.append(more)
        }
        page.navigationItem.rightBarButtonItems = items
    }
}


private extension Notification.Name {
    static let vaultChanged = Notification.Name("DatalithVaultChanged")
}

@_cdecl("datalith_refresh_files")
@MainActor
func refreshFiles() {
    DispatchQueue.main.async { NotificationCenter.default.post(name: .vaultChanged, object: nil) }
}
