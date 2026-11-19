package build.mycelium.datalith;

import android.os.Bundle;
import dev.gpui.mobile.GpuiInputActivity;

/** The selected document is rendered by the same GPUI handler as on iOS and desktop. */
public final class DocumentActivity extends GpuiInputActivity {
    static { System.loadLibrary("datalith_android"); }
    private static native void selectDocument(String path);

    @Override protected void onCreate(Bundle state) {
        String path = getIntent().getStringExtra("document");
        if (path != null) selectDocument(path);
        super.onCreate(state);
    }
}
