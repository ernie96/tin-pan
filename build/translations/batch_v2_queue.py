import json

known = json.load(open("TinPan/Localization/translations.json", encoding="utf-8"))


def grown(old_end, new_end, ends):
    """A help text with a sentence added: the existing translations, changed only where (old, new) in `ends` say."""
    done = next((k for k in known if new_end in k), None)
    if done:
        # Merged already (a rerun after fixing access keys).
        return done, known[done]
    key = next(k for k in known if old_end in k)
    return key.replace(old_end, new_end), [text.replace(old, new) for text, (old, new) in zip(known[key], ends)]


T = dict([
    grown(
        "Ctrl+G opens a Suno link. F5 refreshes.",
        "Ctrl+G opens a Suno link, Ctrl+Shift+E searches all of Suno, and Up next holds songs you queued. F5 refreshes.",
        [
            ("Ctrl+G deschide un link Suno. F5", "Ctrl+G deschide un link Suno, Ctrl+Shift+E caută în tot Suno, iar Urmează păstrează melodiile puse la coadă. F5"),
            ("Ctrl+G відкриває посилання Suno. F5", "Ctrl+G відкриває посилання Suno, Ctrl+Shift+E шукає по всьому Suno, а «Далі» містить пісні, які ви додали в чергу. F5"),
            ("Ctrl+G abre un enlace de Suno. F5", "Ctrl+G abre un enlace de Suno, Ctrl+Shift+E busca en todo Suno y A continuación guarda las canciones que pusiste en cola. F5"),
        ],
    ),
    grown(
        "Ctrl+G opens a song from a Suno link someone sent you, the same way.",
        "Ctrl+G opens a song from a Suno link someone sent you, the same way, and Ctrl+Shift+E searches all of Suno.\nUp next, in the Discover tab, holds songs to play after the current one: Ctrl+Shift+A adds the selected song, Play next in the Song menu puts it first, and Delete there takes one out. When Up next is empty, the list you were playing carries on. With \"When a list ends, keep playing songs Suno picks\" in Settings, Suno's picks follow the last song of a list.",
        [
            ("Ctrl+G deschide la fel o melodie dintr-un link Suno primit de la cineva.",
             "Ctrl+G deschide la fel o melodie dintr-un link Suno primit de la cineva, iar Ctrl+Shift+E caută în tot Suno.\nUrmează, în fila Descoperă, păstrează melodiile de redat după cea curentă: Ctrl+Shift+A adaugă melodia selectată, Redă-o prima în șir din meniul Melodie o pune prima, iar Delete acolo scoate una. Când Urmează e gol, lista pe care o ascultai continuă. Cu „Când se termină o listă, continuă cu melodii alese de Suno” din Setări, alegerile Suno urmează după ultima melodie dintr-o listă."),
            ("Ctrl+G так само відкриває пісню з посилання Suno, яке вам надіслали.",
             "Ctrl+G так само відкриває пісню з посилання Suno, яке вам надіслали, а Ctrl+Shift+E шукає по всьому Suno.\n«Далі» у вкладці Відкриття містить пісні, які заграють після поточної: Ctrl+Shift+A додає вибрану пісню, «Грати наступною» в меню «Пісня» ставить її першою, а Delete там прибирає пісню. Коли «Далі» порожнє, список, який ви слухали, грає далі. Якщо в Налаштуваннях увімкнено «Коли список закінчиться, грати пісні, які вибирає Suno», після останньої пісні списку грають пісні від Suno."),
            ("Ctrl+G abre del mismo modo una canción desde un enlace de Suno que te hayan enviado.",
             "Ctrl+G abre del mismo modo una canción desde un enlace de Suno que te hayan enviado, y Ctrl+Shift+E busca en todo Suno.\nA continuación, en la pestaña Descubrir, guarda canciones para después de la actual: Ctrl+Shift+A añade la canción seleccionada, Escuchar justo después del menú Canción la pone primera y Suprimir allí quita una. Cuando A continuación está vacía, sigue la lista que escuchabas. Con «Al terminar una lista, seguir con canciones que elige Suno» en Ajustes, las elegidas por Suno siguen a la última canción de una lista."),
        ],
    ),
])

T.update({
    "Clear Up ne_xt": ["_Golește lista Urmează", "_Очистити «Далі»", "Eli_minar todo de A continuación"],
    "Add to _queue": ["Adau_gă la coadă", "Додати в _чергу", "A_ñadir a la cola"],
    "Play ne_xt": ["Redă-o prima în _șir", "_Грати наступною", "Escuchar _justo después"],
    "_Search Suno...": ["Caută în _Suno...", "_Шукати в Suno...", "_Buscar en Suno..."],
    "_Up next": ["Ur_mează", "_Далі", "_A continuación"],
    "Up next": ["Urmează", "Далі", "A continuación"],
    "Search Suno": ["Caută în Suno", "Шукати в Suno", "Buscar en Suno"],
    "Finds songs from everyone on Suno, not only yours. They open as a list in the Discover tab.": [
        "Găsește melodii de la toți cei de pe Suno, nu doar ale tale. Se deschid ca listă în fila Descoperă.",
        "Знаходить пісні всіх користувачів Suno, а не лише ваші. Вони відкриваються як список у вкладці Відкриття.",
        "Encuentra canciones de todo Suno, no solo las tuyas. Se abren como una lista en la pestaña Descubrir.",
    ],
    "Searching.": ["Se caută.", "Шукаю.", "Buscando."],
    "_Search for": ["_Caută", "_Шукати", "_Buscar"],
    "Suno found no songs for {0}.": ["Suno nu a găsit melodii pentru {0}.", "Suno не знайшов пісень за запитом {0}.", "Suno no encontró canciones para {0}."],
    "Suno search: {0}": ["Căutare Suno: {0}", "Пошук Suno: {0}", "Búsqueda en Suno: {0}"],
    "{0} can't be played yet.": ["{0} nu poate fi redată încă.", "{0} ще не можна відтворити.", "{0} aún no se puede reproducir."],
    "{0} plays next.": ["{0} se redă următoarea.", "{0} гратиме наступною.", "{0} suena a continuación."],
    "Added {0} to Up next.": ["{0} a fost adăugată la Urmează.", "{0} додано до «Далі».", "{0} añadida a A continuación."],
    "Removed {0} from Up next.": ["{0} a fost scoasă din Urmează.", "{0} прибрано з «Далі».", "{0} quitada de A continuación."],
    "Up next is empty.": ["Urmează e gol.", "«Далі» порожнє.", "A continuación está vacía."],
    "Up next cleared.": ["Urmează a fost golit.", "«Далі» очищено.", "A continuación vaciada."],
    "When a list ends, keep playing songs Suno picks": [
        "Când se termină o listă, continuă cu melodii alese de Suno",
        "Коли список закінчиться, грати пісні, які вибирає Suno",
        "Al terminar una lista, seguir con canciones que elige Suno",
    ],
})
