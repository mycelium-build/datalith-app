package build.mycelium.datalith;

import android.content.ContentResolver;
import android.database.Cursor;
import android.net.Uri;
import android.provider.DocumentsContract;
import android.provider.DocumentsContract.Document;
import org.json.JSONArray;
import java.io.ByteArrayOutputStream;
import java.io.FileNotFoundException;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;

/** Ids are opaque, including on cloud providers. Resolve path components by
 * querying the provider and comparing display names. */
final class DocumentTree {
    private final ContentResolver resolver;
    private final Uri tree;
    private final Uri root;

    DocumentTree(ContentResolver resolver, Uri tree) {
        this.resolver = resolver;
        this.tree = tree;
        this.root = doc(DocumentsContract.getTreeDocumentId(tree));
    }

    private Uri doc(String id) { return DocumentsContract.buildDocumentUriUsingTree(tree, id); }
    private Cursor query(Uri uri, String... columns) throws IOException {
        Cursor cursor = resolver.query(uri, columns, null, null, null);
        if (cursor == null) throw new IOException("Folder provider unavailable");
        return cursor;
    }

    private Cursor children(Uri dir) throws IOException {
        Uri list = DocumentsContract.buildChildDocumentsUriUsingTree(tree, DocumentsContract.getDocumentId(dir));
        Cursor cursor = query(list, Document.COLUMN_DISPLAY_NAME, Document.COLUMN_DOCUMENT_ID);
        if (cursor.getExtras().getBoolean(DocumentsContract.EXTRA_LOADING, false)) {
            cursor.close();
            throw new IOException("Folder contents are still downloading");
        }
        return cursor;
    }

    private Uri child(Uri parent, String name) throws IOException {
        validate(name);
        Uri found = null;
        try (Cursor cursor = children(parent)) {
            while (cursor.moveToNext()) {
                if (name.equals(cursor.getString(0))) {
                    if (found != null) throw new IOException("Multiple documents named " + name);
                    found = doc(cursor.getString(1));
                }
            }
        }
        return found;
    }

    private static void validate(String name) throws IOException {
        if (name.isEmpty() || name.equals(".") || name.equals("..") || name.contains("/"))
            throw new IOException("Invalid document name");
    }

    private Uri locate(String path) throws IOException {
        Uri current = root;
        if (!path.isEmpty()) {
            for (String name : path.split("/", -1)) {
                current = child(current, name);
                if (current == null) throw new FileNotFoundException(path);
            }
        }
        return current;
    }

    private static String name(String path) { return path.substring(path.lastIndexOf('/') + 1); }
    private Uri parent(String path) throws IOException {
        int slash = path.lastIndexOf('/');
        return locate(slash < 0 ? "" : path.substring(0, slash));
    }

    private byte[] read(Uri uri) throws IOException {
        try (InputStream input = resolver.openInputStream(uri);
             ByteArrayOutputStream output = new ByteArrayOutputStream()) {
            if (input == null) throw new IOException("Document unavailable");
            byte[] buffer = new byte[16384];
            for (int n; (n = input.read(buffer)) != -1;) output.write(buffer, 0, n);
            return output.toByteArray();
        }
    }

    private Uri create(Uri parent, String name, String type) throws IOException {
        if (child(parent, name) != null) throw new IOException("Name already in use");
        Uri created = DocumentsContract.createDocument(resolver, parent, type, name);
        if (created == null) throw new IOException("Provider refused creation");
        // A provider may append an extension or number a duplicate. Never
        // silently bind our logical name to someone else's file.
        try (Cursor c = query(created, Document.COLUMN_DISPLAY_NAME)) {
            if (c.moveToFirst() && name.equals(c.getString(0))) return created;
        }
        DocumentsContract.deleteDocument(resolver, created);
        throw new IOException("The provider could not create the requested name");
    }

    byte[] perform(String op, String path, String to, byte[] expected, byte[] content) throws Exception {
        switch (op) {
            case "list": {
                JSONArray names = new JSONArray();
                try (Cursor c = children(locate(path))) {
                    while (c.moveToNext()) { String n = c.getString(0); validate(n); names.put(n); }
                }
                return names.toString().getBytes(StandardCharsets.UTF_8);
            }
            case "attributes": {
                try (Cursor cursor = query(locate(path), Document.COLUMN_SIZE, Document.COLUMN_LAST_MODIFIED)) {
                    if (!cursor.moveToFirst()) throw new FileNotFoundException(path);
                    return new JSONArray().put(cursor.getLong(0)).put(cursor.getLong(1)).toString().getBytes(StandardCharsets.UTF_8);
                }
            }
            case "name": {
                try (Cursor c=query(locate(path),Document.COLUMN_DISPLAY_NAME)) {
                    if (!c.moveToFirst()) throw new FileNotFoundException(path);
                    return c.getString(0).getBytes(StandardCharsets.UTF_8);
                }
            }
            case "directory": {
                try (Cursor c = query(locate(path), Document.COLUMN_MIME_TYPE)) {
                    if (!c.moveToFirst()) throw new FileNotFoundException(path);
                    return new byte[]{ (byte)(Document.MIME_TYPE_DIR.equals(c.getString(0)) ? 1 : 0) };
                }
            }
            case "read": return read(locate(path));
            case "mkdir": create(parent(path), name(path), Document.MIME_TYPE_DIR); break;
            case "save": {
                Uri item;
                if (expected == null) item = create(parent(path), name(path), "application/octet-stream");
                else {
                    item = locate(path);
                    if (!Arrays.equals(expected, read(item))) throw new IOException("Document changed in another app");
                }
                try (OutputStream output = resolver.openOutputStream(item, "wt")) {
                    if (output == null) throw new IOException("Document unavailable");
                    output.write(content);
                }
                break;
            }
            case "rename": {
                Uri old = locate(path), source = parent(path), destination = parent(to);
                if (child(destination, name(to)) != null) throw new IOException("Name already in use");
                if (source.equals(destination)) {
                    if (DocumentsContract.renameDocument(resolver, old, name(to)) == null)
                        throw new IOException("Provider refused rename");
                } else {
                    if (!name(path).equals(name(to))) throw new IOException("Move and rename separately");
                    if (DocumentsContract.moveDocument(resolver, old, source, destination) == null)
                        throw new IOException("Provider refused move");
                }
                break;
            }
            default: throw new IOException("Unknown document operation");
        }
        return new byte[0];
    }
}
