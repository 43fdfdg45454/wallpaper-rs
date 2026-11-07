package io.github.wallpaper_rs

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class FrameCapTest {
    private fun kept(fps: Int, cap: Int, seconds: Int = 10, speed: Float = 1f): Int {
        val frames = FrameCap().apply { this.fps = cap; this.speed = speed }
        return (0 until fps * seconds).count { frames.keeps(it * 1_000_000L / fps) }
    }

    @Test
    fun aCapKeepsAboutThatManyFramesAWallClockSecond() {
        assertTrue(kept(60, 30) in 298..302)
        assertTrue(kept(60, 10) in 98..102)
        assertTrue(kept(50, 30) in 298..302)
        assertTrue(kept(60, 24) in 238..242)
    }

    @Test
    fun noCapAndSlowerVideosKeepEveryFrame() {
        assertEquals(600, kept(60, 0))
        assertEquals(240, kept(24, 30))
        assertEquals(300, kept(30, 60))
    }

    @Test
    fun fasterPlaybackCountsTheCapOnTheScreenNotInTheVideo() {
        assertTrue(kept(60, 30, speed = 2f) in 148..152)
    }

    @Test
    fun askingTwiceAboutTheSameFrameGivesTheSameAnswer() {
        val frames = FrameCap().apply { fps = 10 }
        val first = (0 until 12).map { frames.keeps(it * 16_667L) to frames.keeps(it * 16_667L) }
        assertTrue(first.all { it.first == it.second })
    }

    @Test
    fun aLoopOrASeekShowsItsFirstFrameAtOnce() {
        val frames = FrameCap().apply { fps = 10 }
        assertTrue((0 until 30).map { frames.keeps(it * 16_667L) }.first())
        frames.reset()
        assertTrue(frames.keeps(5 * 16_667L))
        assertTrue(!frames.keeps(6 * 16_667L))
        assertTrue(frames.keeps(0L))
    }
}
