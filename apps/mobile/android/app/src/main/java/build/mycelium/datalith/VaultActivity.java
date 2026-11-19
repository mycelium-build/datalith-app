package build.mycelium.datalith;

import android.app.ListActivity;
import android.content.Intent;
import android.os.Bundle;
import android.view.View;
import android.widget.ArrayAdapter;
import android.widget.ListView;
import android.widget.Toast;

import java.io.File;
import java.util.Arrays;
import java.util.Comparator;

/** Each folder is a native navigation destination. Android restores the list and its scroll. */
public class VaultActivity extends ListActivity {
    static { System.loadLibrary("datalith_android"); }
    private static native String prepareVault(String documents);
    private File folder;
    private File[] entries = new File[0];

    @Override protected void onCreate(Bundle state) {
        super.onCreate(state);
        File documents = new File(getFilesDir(), "Documents");
        String root = prepareVault(documents.getPath());
        if (root == null) {
            Toast.makeText(this, "Could not open the vault", Toast.LENGTH_LONG).show();
            finish();
            return;
        }
        String child = getClass() == FolderActivity.class ? getIntent().getStringExtra("folder") : null;
        try {
            File canonicalRoot = new File(root).getCanonicalFile();
            folder = new File(canonicalRoot, child == null ? "" : child).getCanonicalFile();
            if (!folder.toPath().startsWith(canonicalRoot.toPath())) folder = canonicalRoot;
        } catch (java.io.IOException error) {
            finish();
            return;
        }
        setTitle(folder.getName());
    }

    @Override protected void onResume() {
        super.onResume();
        if (folder == null) return;
        File[] files = folder.listFiles(file -> !file.isHidden());
        entries = files == null ? new File[0] : files;
        Arrays.sort(entries, Comparator.comparing(File::getName, String.CASE_INSENSITIVE_ORDER));
        String[] labels = Arrays.stream(entries).map(File::getName).toArray(String[]::new);
        setListAdapter(new ArrayAdapter<>(this, android.R.layout.simple_list_item_1, labels));
    }

    @Override protected void onListItemClick(ListView list, View view, int position, long id) {
        File selected = entries[position];
        if (selected.isDirectory()) {
            String parent = getClass() == FolderActivity.class ? getIntent().getStringExtra("folder") : null;
            String relative = parent == null ? selected.getName() : parent + "/" + selected.getName();
            startActivity(new Intent(this, FolderActivity.class).putExtra("folder", relative));
        } else {
            startActivity(new Intent(this, DocumentActivity.class).putExtra("document", selected.getPath()));
        }
    }
}
