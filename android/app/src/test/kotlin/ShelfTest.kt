package io.github.wallpaper_rs

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ShelfTest {
    @Test
    fun theRepositoryIsMirroredWithoutItsPackagesOrPreviews() {
        assertTrue(shelved("catalog.json"))
        assertTrue(shelved("root/1.json.minisig"))
        assertTrue(shelved("entry/latest"))
        assertFalse(shelved("packages/org.example.dawn/1.0.0.wpak"))
        assertFalse(shelved("previews/0a.png"))
        assertTrue(shelved("index/packages.json"))
    }

    @Test
    fun onlyPathsInsideTheFolderAreWrittenBack() {
        assertTrue(safe("packages/org.example.dawn/1.0.0.wpak"))
        assertFalse(safe("../outside.json"))
        assertFalse(safe("entry/../../outside.json"))
        assertFalse(safe("/catalog.json"))
        assertFalse(safe("root//1.json"))
        assertFalse(shelved("./catalog.json"))
    }
}
