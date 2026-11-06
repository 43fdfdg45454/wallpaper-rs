package io.github.wallpaper_rs

data class MediaShown(
    val listening: Boolean = false,
    val players: Int = 0,
    val track: Boolean = false,
    val playing: Boolean = false,
    val cover: Boolean = false,
) {
    companion object {
        fun of(all: Collection<MediaShown>): MediaShown =
            all.filter { it.listening }.maxWithOrNull(compareBy({ it.track }, { it.playing }, { it.players })) ?: MediaShown()
    }
}
