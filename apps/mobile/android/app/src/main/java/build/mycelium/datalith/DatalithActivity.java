package build.mycelium.datalith;

import android.app.Activity;
import android.content.Intent;
import android.net.Uri;
import dev.gpui.mobile.GpuiInputActivity;

/** Native host for the system folder picker. All document I/O uses the returned grant. */
public final class DatalithActivity extends GpuiInputActivity {
    private static final int PICK_FOLDER = 841;
    private static native void folderPicked(String uri, String error);

    private static native void openLink(String url);
    private static native void choseOption(int index);
    /** Gives Rust this Activity: the JNI context GPUI exposes is the Application. */
    private static native void attach(DatalithActivity activity);

    @Override protected void onCreate(android.os.Bundle state) {
        super.onCreate(state);
        attach(this);
        if (state == null) deliverLink(getIntent());
    }
    @Override protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        deliverLink(intent);
    }
    private void deliverLink(Intent intent) {
        if (Intent.ACTION_VIEW.equals(intent.getAction()) && intent.getData() != null
                && "datalith".equals(intent.getData().getScheme())) openLink(intent.getDataString());
    }

    /** Body text size for the person's font size setting: 16 sp at the default scale. */
    public float preferredTextSize() {
        return 16f * getResources().getConfiguration().fontScale;
    }

    /** Whether the app shows a light theme; dialogs follow it. */
    private volatile boolean lightTheme = true;

    /** The device's own alert style (One UI on Samsung), light or dark like the app. */
    private int dialogTheme() {
        return lightTheme ? android.R.style.Theme_DeviceDefault_Light_Dialog_Alert
            : android.R.style.Theme_DeviceDefault_Dialog_Alert;
    }

    /** Paints the status and navigation bars with the theme background and picks icons that contrast with it. */
    public void setSystemBars(int background, boolean lightBackground) {
        lightTheme = lightBackground;
        runOnUiThread(() -> {
            android.view.Window window = getWindow();
            window.setStatusBarColor(background);
            window.setNavigationBarColor(background);
            android.view.WindowInsetsController controller = window.getInsetsController();
            if (controller == null) return;
            int light = android.view.WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS
                | android.view.WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS;
            controller.setSystemBarsAppearance(lightBackground ? light : 0, light);
        });
    }

    public void chooseOption(String title, String[] options, int selected) {
        runOnUiThread(() -> new android.app.AlertDialog.Builder(this, dialogTheme())
            .setTitle(title)
            .setSingleChoiceItems(options, selected, (dialog, index) -> {
                dialog.dismiss();
                choseOption(index);
            })
            .setNegativeButton(android.R.string.cancel, (dialog, button) -> choseOption(-1))
            .setOnCancelListener(dialog -> choseOption(-1)).show());
    }

    /** Commands on one object. Rows are actions, so no radio buttons; the destructive one is tinted. */
    public void chooseActions(String title, String[] actions, int destructive) {
        runOnUiThread(() -> {
            CharSequence[] rows = new CharSequence[actions.length];
            for (int i = 0; i < actions.length; i++) {
                if (i == destructive) {
                    android.text.SpannableString text = new android.text.SpannableString(actions[i]);
                    android.util.TypedValue error = new android.util.TypedValue();
                    int color = getTheme().resolveAttribute(android.R.attr.colorError, error, true)
                        ? error.data : 0xFFB3261E;
                    text.setSpan(new android.text.style.ForegroundColorSpan(color), 0, text.length(), 0);
                    rows[i] = text;
                } else {
                    rows[i] = actions[i];
                }
            }
            new android.app.AlertDialog.Builder(this, dialogTheme())
                .setTitle(title)
                .setItems(rows, (dialog, index) -> choseOption(index))
                .setNegativeButton(android.R.string.cancel, (dialog, button) -> choseOption(-1))
                .setOnCancelListener(dialog -> choseOption(-1)).show();
        });
    }

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
