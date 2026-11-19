package build.mycelium.datalith;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.database.Cursor;
import android.database.MatrixCursor;
import android.net.Uri;
import android.os.CancellationSignal;
import android.os.ParcelFileDescriptor;
import android.provider.DocumentsContract;
import android.provider.DocumentsProvider;
import java.io.File;
import java.io.FileNotFoundException;
import java.io.IOException;
import java.util.UUID;

/** Only packaged in the instrumentation APK. A separate UID owns the grant
 * and the documents, as with an installed third-party storage provider. */
public final class TestDocuments extends DocumentsProvider {
    static final String AUTHORITY = "build.mycelium.datalith.test.documents";
    static final int ACCESS = Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_WRITE_URI_PERMISSION;
    public static final class Grant extends BroadcastReceiver {
        @Override public void onReceive(Context context, Intent ignored) {
            String id = UUID.randomUUID().toString();
            File folder = new File(context.getCacheDir(), id);
            if (!folder.mkdir()) return;
            Uri uri = DocumentsContract.buildTreeDocumentUri(AUTHORITY, id);
            context.grantUriPermission("build.mycelium.datalith", uri, ACCESS
                | Intent.FLAG_GRANT_PREFIX_URI_PERMISSION | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION);
            setResultData(uri.toString());
        }
    }

    @Override public boolean onCreate() { return true; }
    @Override public Cursor queryRoots(String[] projection) { return new MatrixCursor(projection); }
    private File file(String id) throws FileNotFoundException {
        try {
            File base = getContext().getCacheDir().getCanonicalFile();
            File f = new File(base,id).getCanonicalFile();
            if (f.toPath().startsWith(base.toPath()) && f.exists()) return f;
        } catch (IOException ignored) { }
        throw new FileNotFoundException(id);
    }
    private String id(File f) { try {return getContext().getCacheDir().getCanonicalFile().toPath().relativize(f.getCanonicalFile().toPath()).toString();}catch(IOException e){throw new IllegalStateException(e);} }
    private void row(MatrixCursor c, File f) {
        MatrixCursor.RowBuilder r = c.newRow();
        for (String key : c.getColumnNames()) {
            switch (key) {
                case DocumentsContract.Document.COLUMN_DOCUMENT_ID: r.add(id(f)); break;
                case DocumentsContract.Document.COLUMN_MIME_TYPE: r.add(f.isDirectory()?DocumentsContract.Document.MIME_TYPE_DIR:"text/plain"); break;
                case DocumentsContract.Document.COLUMN_DISPLAY_NAME: r.add(f.getName()); break;
                default: r.add(null);
            }
        }
    }
    @Override public Cursor queryDocument(String id,String[] projection) throws FileNotFoundException {
        MatrixCursor c = new MatrixCursor(projection); row(c,file(id)); return c;
    }
    @Override public Cursor queryChildDocuments(String id,String[] projection,String sort) throws FileNotFoundException {
        MatrixCursor c = new MatrixCursor(projection);
        File[] children = file(id).listFiles();
        if (children == null) throw new FileNotFoundException(id);
        for (File f:children) row(c,f);
        return c;
    }
    @Override public boolean isChildDocument(String parent,String child) { return child.startsWith(parent+"/"); }
    @Override public ParcelFileDescriptor openDocument(String id,String mode,CancellationSignal ignored) throws FileNotFoundException {
        return ParcelFileDescriptor.open(file(id),ParcelFileDescriptor.parseMode(mode));
    }
    @Override public String createDocument(String parent,String type,String name) throws FileNotFoundException {
        if (name.contains("/") || name.equals("..")) throw new FileNotFoundException(name);
        File f=new File(file(parent),name);
        try { if (type.equals(DocumentsContract.Document.MIME_TYPE_DIR)?f.mkdir():f.createNewFile()) return id(f); }
        catch(IOException ignored) { }
        throw new FileNotFoundException(name);
    }
    @Override public String moveDocument(String id,String from,String to) throws FileNotFoundException {
        File src=file(id),dest=new File(file(to),src.getName());
        if (!src.renameTo(dest)) throw new FileNotFoundException(id);
        return id(dest);
    }
    @Override public void deleteDocument(String id) throws FileNotFoundException {
        File f=file(id);
        File[] kids=f.listFiles();
        if (kids!=null) for(File kid:kids) deleteDocument(id(kid));
        if(!f.delete()) throw new FileNotFoundException(id);
    }
}
