import json, os

# The F1 text is the views batch's, with Discover's lists in its second sentence.
_here = os.path.dirname(os.path.abspath(__file__))
_old = json.load(open(os.path.join(_here, '..', '..', 'TinPan', 'Localization', 'translations.json'), encoding='utf-8'))
_OLD_EN = "Ctrl+1: Library, your songs and playlists. Ctrl+2: Discover, with Trending."
_NEW_EN = "Ctrl+1: Library, your songs and playlists. Ctrl+2: Discover: Trending, From people you follow, Explore's shelves and Contests; lists opened from a song (similar songs, its creator's songs, covers and remixes, the original) join them, and Delete closes one."
_help_key = next(k for k in _old if k.startswith(_OLD_EN))
_swaps = [
    ("Ctrl+2: Descoperă, cu În tendințe.", "Ctrl+2: Descoperă: În tendințe, De la oamenii pe care îi urmăriți, rafturile din Explorează și Concursuri; listele deschise de la o melodie (melodii asemănătoare, melodiile creatorului, preluări și remixuri, originalul) li se alătură, iar Delete închide una."),
    ("Ctrl+2: Відкриття, з «У тренді».", "Ctrl+2: Відкриття: «У тренді», «Від тих, за ким ви стежите», полиці Огляду й Конкурси; списки, відкриті з пісні (схожі пісні, пісні автора, кавери й ремікси, оригінал), додаються до них, а Delete закриває список."),
    ("Ctrl+2: Descubrir, con Tendencias.", "Ctrl+2: Descubrir: Tendencias, De las personas que sigue, los estantes de Explorar y Concursos; las listas abiertas desde una canción (canciones parecidas, las del creador, versiones y remixes, la original) se suman a ellas, y Suprimir cierra una."),
]
_help = []
for text, (old, new) in zip(_old[_help_key], _swaps):
    assert old in text, old
    _help.append(text.replace(old, new))

T = {
    "Explore": ["Explorează", "Огляд", "Explorar"],
    "Closed {0}.": ["Am închis {0}.", "Закрито: {0}.", "Se cerró {0}."],
    _NEW_EN + _help_key[len(_OLD_EN):]: _help,
}
