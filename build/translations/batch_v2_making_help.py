import json, os

# The F1 text as it was, with a sentence about the making forms and the two new tabs after Ctrl+N's.
_here = os.path.dirname(os.path.abspath(__file__))
_old = json.load(open(os.path.join(_here, '..', '..', 'TinPan', 'Localization', 'translations.json'), encoding='utf-8'))
_N = "Ctrl+N: make a song, in a window that stays open; Escape hides it. "
_NEW = "Ctrl+E, Ctrl+Shift+C and Ctrl+Shift+M: extend, cover or remaster the selected song, in forms that stay open. Ctrl+4: Account. Ctrl+5: Settings. "
_key = next(k for k in _old if k.startswith("Ctrl+1: Library, your songs and playlists.") and _N in k)
_added = [
    ("Escape o ascunde. ", "Ctrl+E, Ctrl+Shift+C și Ctrl+Shift+M: extinde, preia sau remasterizează melodia aleasă, în formulare care rămân deschise. Ctrl+4: Cont. Ctrl+5: Setări. "),
    ("Escape ховає його. ", "Ctrl+E, Ctrl+Shift+C і Ctrl+Shift+M: продовжити, зробити кавер або ремастер вибраної пісні у формах, що лишаються відкритими. Ctrl+4: Обліковий запис. Ctrl+5: Налаштування. "),
    ("Escape la oculta. ", "Ctrl+E, Ctrl+Mayús+C y Ctrl+Mayús+M: extender, versionar o remasterizar la canción elegida, en formularios que siguen abiertos. Ctrl+4: Cuenta. Ctrl+5: Configuración. "),
]
_help = []
for text, (anchor, sentence) in zip(_old[_key], _added):
    assert text.count(anchor) == 1, anchor
    _help.append(text.replace(anchor, anchor + sentence))

T = {_key.replace(_N, _N + _NEW): _help}
