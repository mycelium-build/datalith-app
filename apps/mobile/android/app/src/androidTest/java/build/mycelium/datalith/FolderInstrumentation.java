package build.mycelium.datalith;

import android.app.Activity;
import android.app.Instrumentation;
import android.content.BroadcastReceiver;
import android.content.ContentResolver;
import android.content.Context;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.provider.DocumentsContract;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.TimeUnit;

/** Exercises a separate app's DocumentsProvider. Run first with "-e phase write"
 * and again with "-e phase reopen" to check grant survival across process death. */
public final class FolderInstrumentation extends Instrumentation {
    private String phase;

    @Override public void onCreate(Bundle arguments) {
        super.onCreate(arguments);
        phase = arguments.getString("phase", "both");
        start();
    }

    @Override public void onStart() {
        Bundle result = new Bundle();
        Uri grant = null;
        ContentResolver resolver = getTargetContext().getContentResolver();
        android.content.SharedPreferences fixture = getTargetContext().getSharedPreferences("saf-test", Context.MODE_PRIVATE);
        try {
            if ("reopen".equals(phase)) {
                String saved = fixture.getString("grant", null);
                if (saved == null) throw new AssertionError("Run the write phase first");
                grant = Uri.parse(saved);
            } else {
                grant = requestGrant();
                resolver.takePersistableUriPermission(grant, TestDocuments.ACCESS);
                writeFixture(new DocumentTree(resolver, grant));
                if (!fixture.edit().putString("grant", grant.toString()).commit()) throw new IOException("Grant fixture unavailable");
            }

            Uri expected = grant;
            if (!resolver.getPersistedUriPermissions().stream().anyMatch(p -> p.isWritePermission() && expected.equals(p.getUri())))
                throw new AssertionError("Missing persisted write grant");

            DocumentTree tree = new DocumentTree(resolver, grant);
            if (!Arrays.equals(bytes("coffee"), tree.perform("read", "Notes/Courses.md", null, null, null))
                || !Arrays.equals(bytes("bread"), tree.perform("read", "Courses (local edits).md", null, null, null)))
                throw new AssertionError("Unexpected content after reopening the tree");
            result.putString("stream", "DocumentTree " + phase + " passed\n");
        } catch (Exception | AssertionError error) {
            result.putString("shortMsg", android.util.Log.getStackTraceString(error));
        } finally {
            if (grant != null && (!"write".equals(phase) || result.containsKey("shortMsg"))) {
                try {
                    Uri root = DocumentsContract.buildDocumentUriUsingTree(grant, DocumentsContract.getTreeDocumentId(grant));
                    DocumentsContract.deleteDocument(resolver, root);
                    fixture.edit().remove("grant").apply();
                } catch (Exception error) { result.putString("shortMsg", error.toString()); }
            }
        }
        finish(result.containsKey("shortMsg") ? Activity.RESULT_CANCELED : Activity.RESULT_OK, result);
    }

    private Uri requestGrant() throws Exception {
        CompletableFuture<String> issued = new CompletableFuture<>();
        Intent request = new Intent().setClassName("build.mycelium.datalith.test", TestDocuments.Grant.class.getName());
        getTargetContext().sendOrderedBroadcast(request, null, new BroadcastReceiver() {
            @Override public void onReceive(Context context, Intent intent) { issued.complete(getResultData()); }
        }, null, 0, null, null);
        return Uri.parse(issued.get(12, TimeUnit.SECONDS));
    }

    private static void writeFixture(DocumentTree tree) throws Exception {
        byte[] initial = bytes("milk"), remote = bytes("coffee"), draft = bytes("bread");
        tree.perform("save", "Courses.md", null, null, initial);
        tree.perform("save", "Courses.md", null, initial, remote);
        try {
            tree.perform("save", "Courses.md", null, initial, draft);
            throw new AssertionError("Stale write accepted");
        } catch (IOException conflict) {
            if (!conflict.getMessage().contains("changed in another app")) throw conflict;
        }
        tree.perform("save", "Courses (local edits).md", null, null, draft);
        tree.perform("mkdir", "Notes", null, null, null);
        tree.perform("rename", "Courses.md", "Notes/Courses.md", null, null);
    }

    private static byte[] bytes(String text) { return text.getBytes(StandardCharsets.UTF_8); }
}
