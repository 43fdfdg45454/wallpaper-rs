package io.github.wallpaper_rs;

import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.time.Duration;
import java.time.Instant;
import java.time.temporal.ChronoUnit;
import org.json.JSONException;
import org.json.JSONObject;

public final class Anchor {
    private Anchor() {}

    public static File renew(File root) throws IOException {
        try {
            JSONObject json = new JSONObject(new String(Files.readAllBytes(root.toPath()), StandardCharsets.UTF_8));
            json.put("expires", Instant.now().plus(Duration.ofDays(365)).truncatedTo(ChronoUnit.SECONDS).toString());
            Files.write(root.toPath(), json.toString().getBytes(StandardCharsets.UTF_8));
            return root;
        } catch (JSONException e) {
            throw new IOException(e);
        }
    }
}
