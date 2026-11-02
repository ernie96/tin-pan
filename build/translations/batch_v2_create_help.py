import json, os

# The F1 text as it was, with the Ctrl+N sentence before the message keys.
_here = os.path.dirname(os.path.abspath(__file__))
_old = json.load(open(os.path.join(_here, '..', '..', 'TinPan', 'Localization', 'translations.json'), encoding='utf-8'))
_TAIL = "Ctrl+R repeats the last message, Ctrl+Shift+R shows them all. Alt opens the menus."
_key = next(k for k in _old if k.startswith("Ctrl+1: Library, your songs and playlists.") and k.endswith(_TAIL))
_added = [
    ("Ctrl+R repetă", "Ctrl+N: fă o melodie, într-o fereastră care rămâne deschisă; Escape o ascunde. "),
    ("Ctrl+R повторює", "Ctrl+N: створити пісню у вікні, що лишається відкритим; Escape ховає його. "),
    ("Ctrl+R repite", "Ctrl+N: crear una canción, en una ventana que sigue abierta; Escape la oculta. "),
]
_help = []
for text, (anchor, sentence) in zip(_old[_key], _added):
    assert text.count(anchor) == 1, anchor
    _help.append(text.replace(anchor, sentence + anchor))

T = {_key.replace(_TAIL, "Ctrl+N: make a song, in a window that stays open; Escape hides it. " + _TAIL): _help}
