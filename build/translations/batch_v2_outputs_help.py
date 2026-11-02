import json, os

# The F1 text as it was, with Ctrl+Shift+F12 after the volume keys.
_here = os.path.dirname(os.path.abspath(__file__))
_old = json.load(open(os.path.join(_here, '..', '..', 'TinPan', 'Localization', 'translations.json'), encoding='utf-8'))
_V = "F11 and F12: volume. "
_NEW = "Ctrl+Shift+F12: switch to the second audio output and back. "
_key = next(k for k in _old if k.startswith("Ctrl+1: Library, your songs and playlists.") and _V in k and _NEW not in k)
_added = [
    ("F11 și F12: volum. ", "Ctrl+Shift+F12: trece pe a doua ieșire audio și înapoi. "),
    ("F11 і F12: гучність. ", "Ctrl+Shift+F12: перемкнутися на другий аудіовихід і назад. "),
    ("F11 y F12: volumen. ", "Ctrl+Mayús+F12: cambiar a la segunda salida de audio y volver. "),
]
_help = []
for text, (anchor, sentence) in zip(_old[_key], _added):
    assert text.count(anchor) == 1, anchor
    _help.append(text.replace(anchor, anchor + sentence))

T = {_key.replace(_V, _V + _NEW): _help}
