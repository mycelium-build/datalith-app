package build.mycelium.datalith;

import android.app.Activity;
import android.content.Intent;
import android.net.Uri;
import dev.gpui.mobile.GpuiInputActivity;

/** Native host for the system folder picker. All document I/O uses the returned grant. */
public final class DatalithActivity extends GpuiInputActivity {
    private static final int PICK_FOLDER = 841;
    private static native void folderPicked(String uri, String error);

    public void chooseVault() {
        runOnUiThread(() -> {
            try {
                Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE);
                intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_WRITE_URI_PERMISSION
                    | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION | Intent.FLAG_GRANT_PREFIX_URI_PERMISSION);
                startActivityForResult(intent, PICK_FOLDER);
            } catch (RuntimeException error) { folderPicked(null, error.toString()); }
        });
    }

    @Override protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (request != PICK_FOLDER) return;
        if (result != Activity.RESULT_OK || data == null || data.getData() == null) {
            folderPicked(null, null);
            return;
        }
        Uri tree = data.getData();
        try {
            int flags = data.getFlags() & (Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_WRITE_URI_PERMISSION);
            if ((flags & Intent.FLAG_GRANT_WRITE_URI_PERMISSION) == 0) throw new SecurityException("Folder is read-only");
            getContentResolver().takePersistableUriPermission(tree, flags);
            folderPicked(tree.toString(), null);
        } catch (RuntimeException error) { folderPicked(null, error.toString()); }
    }

    public void forgetVault(String grant) {
        Uri uri = Uri.parse(grant);
        for (android.content.UriPermission p : getContentResolver().getPersistedUriPermissions()) {
            if (uri.equals(p.getUri())) getContentResolver().releasePersistableUriPermission(uri,
                (p.isReadPermission() ? Intent.FLAG_GRANT_READ_URI_PERMISSION : 0)
                | (p.isWritePermission() ? Intent.FLAG_GRANT_WRITE_URI_PERMISSION : 0));
        }
    }

    public byte[] vaultOperation(String grant, String op, String relative, String destination,
                                byte[] expected, byte[] content) throws Exception {
        return new DocumentTree(getContentResolver(), Uri.parse(grant)).perform(op, relative, destination, expected, content);
    }
}
