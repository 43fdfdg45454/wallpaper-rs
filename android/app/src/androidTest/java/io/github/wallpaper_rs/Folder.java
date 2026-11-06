package io.github.wallpaper_rs;

import android.database.Cursor;
import android.database.MatrixCursor;
import android.os.CancellationSignal;
import android.os.ParcelFileDescriptor;
import android.provider.DocumentsContract.Document;
import android.provider.DocumentsContract.Root;
import android.provider.DocumentsProvider;
import java.io.File;
import java.io.FileNotFoundException;
import java.io.IOException;
import java.util.Arrays;

public class Folder extends DocumentsProvider {
    static final String REPOSITORY = "repo";
    private static final String[] ROOTS = {Root.COLUMN_ROOT_ID, Root.COLUMN_DOCUMENT_ID, Root.COLUMN_TITLE, Root.COLUMN_FLAGS};
    private static final String[] DOCUMENTS = {Document.COLUMN_DOCUMENT_ID, Document.COLUMN_DISPLAY_NAME, Document.COLUMN_MIME_TYPE, Document.COLUMN_FLAGS, Document.COLUMN_SIZE};

    static File base(android.content.Context context) {
        return new File(context.getFilesDir(), "documents");
    }

    private File file(String id) {
        return new File(base(getContext()), id);
    }

    private String id(File file) {
        return base(getContext()).toURI().relativize(file.toURI()).getPath().replaceAll("/$", "");
    }

    private void row(MatrixCursor cursor, File file) {
        boolean dir = file.isDirectory();
        cursor.newRow().add(Document.COLUMN_DOCUMENT_ID, id(file)).add(Document.COLUMN_DISPLAY_NAME, file.getName())
            .add(Document.COLUMN_MIME_TYPE, dir ? Document.MIME_TYPE_DIR : "application/octet-stream")
            .add(Document.COLUMN_FLAGS, dir ? Document.FLAG_DIR_SUPPORTS_CREATE : Document.FLAG_SUPPORTS_WRITE)
            .add(Document.COLUMN_SIZE, file.length());
    }

    @Override
    public boolean onCreate() {
        return true;
    }

    @Override
    public Cursor queryRoots(String[] projection) {
        MatrixCursor cursor = new MatrixCursor(projection != null ? projection : ROOTS);
        cursor.newRow().add(Root.COLUMN_ROOT_ID, REPOSITORY).add(Root.COLUMN_DOCUMENT_ID, REPOSITORY)
            .add(Root.COLUMN_TITLE, REPOSITORY).add(Root.COLUMN_FLAGS, Root.FLAG_SUPPORTS_CREATE | Root.FLAG_SUPPORTS_IS_CHILD);
        return cursor;
    }

    @Override
    public Cursor queryDocument(String documentId, String[] projection) {
        MatrixCursor cursor = new MatrixCursor(projection != null ? projection : DOCUMENTS);
        row(cursor, file(documentId));
        return cursor;
    }

    @Override
    public Cursor queryChildDocuments(String parentDocumentId, String[] projection, String sortOrder) {
        MatrixCursor cursor = new MatrixCursor(projection != null ? projection : DOCUMENTS);
        File[] children = file(parentDocumentId).listFiles();
        if (children != null) {
            Arrays.sort(children);
            for (File child : children) row(cursor, child);
        }
        return cursor;
    }

    @Override
    public ParcelFileDescriptor openDocument(String documentId, String mode, CancellationSignal signal) throws FileNotFoundException {
        return ParcelFileDescriptor.open(file(documentId), ParcelFileDescriptor.parseMode(mode));
    }

    @Override
    public String createDocument(String parentDocumentId, String mimeType, String displayName) throws FileNotFoundException {
        File made = new File(file(parentDocumentId), displayName);
        boolean done;
        try {
            done = Document.MIME_TYPE_DIR.equals(mimeType) ? made.mkdirs() : made.createNewFile();
        } catch (IOException e) {
            throw new FileNotFoundException(e.getMessage());
        }
        if (!done) throw new FileNotFoundException(displayName);
        return id(made);
    }

    @Override
    public boolean isChildDocument(String parentDocumentId, String documentId) {
        return documentId.startsWith(parentDocumentId + "/");
    }
}
