import json

known = json.load(open("TinPan/Localization/translations.json", encoding="utf-8"))


def grown(old_end, new_end, ends):
    """A help text with a sentence added: the existing translations, changed only where (old, new) in `ends` say."""
    key = next(k for k in known if old_end in k)
    return key.replace(old_end, new_end), [text.replace(old, new) for text, (old, new) in zip(known[key], ends)]


T = dict([
    grown(
        "Delete in the lists closes a list you opened. F5 refreshes.",
        "Delete in the lists closes a list you opened. Ctrl+G opens a Suno link. F5 refreshes.",
        [
            ("deschisă de tine. F5", "deschisă de tine. Ctrl+G deschide un link Suno. F5"),
            ("відкритий вами список. F5", "відкритий вами список. Ctrl+G відкриває посилання Suno. F5"),
            ("que abriste. F5", "que abriste. Ctrl+G abre un enlace de Suno. F5"),
        ],
    ),
    grown(
        "Delete in the lists closes one.",
        "Delete in the lists closes one. Ctrl+G opens a song from a Suno link someone sent you, the same way.",
        [
            ("Delete în liste închide una.", "Delete în liste închide una. Ctrl+G deschide la fel o melodie dintr-un link Suno primit de la cineva."),
            ("Delete у списках закриває такий список.", "Delete у списках закриває такий список. Ctrl+G так само відкриває пісню з посилання Suno, яке вам надіслали."),
            ("Suprimir en las listas cierra una.", "Suprimir en las listas cierra una. Ctrl+G abre del mismo modo una canción desde un enlace de Suno que te hayan enviado."),
        ],
    ),
])

T.update({
    "Open a Suno _link...": ["Deschide un _link Suno...", "Відкрити _посилання Suno...", "Abrir un en_lace de Suno..."],
    "Open a Suno link": ["Deschide un link Suno", "Відкрити посилання Suno", "Abrir un enlace de Suno"],
    "_Link to the song": ["_Linkul melodiei", "_Посилання на пісню", "En_lace de la canción"],
    "Paste a link to a Suno song. It opens as a list in the Discover tab.": [
        "Lipește linkul unei melodii Suno. Se deschide ca listă în fila Descoperă.",
        "Вставте посилання на пісню Suno. Вона відкриється як список у вкладці Відкриття.",
        "Pega el enlace de una canción de Suno. Se abre como una lista en la pestaña Descubrir.",
    ],
    "Open": ["Deschide", "Відкрити", "Abrir"],
    "That isn't a link to a Suno song.": [
        "Acesta nu este linkul unei melodii Suno.",
        "Це не посилання на пісню Suno.",
        "Eso no es un enlace a una canción de Suno.",
    ],
    "Suno couldn't find that song. It may be private or deleted.": [
        "Suno nu a găsit melodia. Poate fi privată sau ștearsă.",
        "Suno не знайшов цю пісню. Можливо, вона приватна або видалена.",
        "Suno no encontró esa canción. Puede que sea privada o se haya eliminado.",
    ],
})
