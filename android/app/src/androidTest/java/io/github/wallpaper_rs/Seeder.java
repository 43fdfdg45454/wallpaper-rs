package io.github.wallpaper_rs;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.provider.DocumentsContract;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;

public class Seeder extends ContentProvider {
    static final String INCOMING = "in/dawn.wpak";
    private static final int GRANTS = Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_WRITE_URI_PERMISSION | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION;

    private void unpack(String path, File to) throws IOException {
        String[] names = getContext().getAssets().list(path);
        if (names == null || names.length == 0) {
            to.getParentFile().mkdirs();
            try (InputStream in = getContext().getAssets().open(path); OutputStream out = new FileOutputStream(to)) {
                byte[] buffer = new byte[65536];
                for (int n; (n = in.read(buffer)) > 0; ) out.write(buffer, 0, n);
            }
            return;
        }
        for (String name : names) unpack(path + "/" + name, new File(to, name));
    }

    private static void remove(File file) {
        File[] children = file.listFiles();
        if (children != null) for (File child : children) remove(child);
        file.delete();
    }

    @Override
    public Bundle call(String method, String arg, Bundle extras) {
        File base = Folder.base(getContext());
        String authority = getContext().getPackageName() + ".folder";
        remove(base);
        try {
            unpack("stall", new File(base, Folder.REPOSITORY));
            File anchor = new File(new File(base, Folder.REPOSITORY), "root/1.json");
            unpack("stall-anchor.json", anchor);
            Anchor.renew(anchor);
            unpack("stall-dawn.wpak", new File(base, INCOMING));
        } catch (IOException e) {
            throw new IllegalStateException(e);
        }
        Uri tree = DocumentsContract.buildTreeDocumentUri(authority, Folder.REPOSITORY);
        getContext().grantUriPermission(arg, tree, GRANTS | Intent.FLAG_GRANT_PREFIX_URI_PERMISSION);
        getContext().grantUriPermission(arg, DocumentsContract.buildDocumentUri(authority, INCOMING), GRANTS);
        return new Bundle();
    }

    @Override
    public boolean onCreate() {
        return true;
    }

    @Override
    public Cursor query(Uri uri, String[] projection, String selection, String[] args, String order) {
        return null;
    }

    @Override
    public String getType(Uri uri) {
        return null;
    }

    @Override
    public Uri insert(Uri uri, ContentValues values) {
        return null;
    }

    @Override
    public int delete(Uri uri, String selection, String[] args) {
        return 0;
    }

    @Override
    public int update(Uri uri, ContentValues values, String selection, String[] args) {
        return 0;
    }
}
