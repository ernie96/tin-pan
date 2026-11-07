import json

known = json.load(open("TinPan/Localization/translations.json", encoding="utf-8"))

OLD_HELP = "Help menu, Report a problem (Ctrl+Shift+B) and Suggest a feature (Ctrl+Shift+F) send a message privately to Tin Pan's author; no account is needed."
ADDED = " Contact the author lists Ernesto's email, Mastodon and the Telegram news channel, and Donate lists ways to support Tin Pan; Enter opens one. The update offer has a Donate button too."
help_key = next(k for k in known if k.startswith(OLD_HELP))
help_added = [
    " Contactează autorul arată emailul lui Ernesto, Mastodon și canalul de știri de pe Telegram, iar Donează arată cum poți sprijini Tin Pan; Enter îl deschide pe cel ales. Oferta de actualizare are și ea un buton Donează.",
    " «Зв’язатися з автором» показує електронну пошту Ернесто, Mastodon і канал новин у Telegram, а «Пожертвувати» показує, як підтримати Tin Pan; Enter відкриває вибране. У пропозиції оновлення теж є кнопка «Пожертвувати».",
    " Contactar con el autor muestra el correo de Ernesto, Mastodon y el canal de noticias de Telegram, y Donar muestra formas de apoyar Tin Pan; Intro abre la elegida. La oferta de actualización también tiene un botón Donar.",
]
first_sentence_ends = ["autorului Tin Pan; nu e nevoie de cont.", "лише авторові Tin Pan; обліковий запис не потрібен.", "autor de Tin Pan; no hace falta cuenta."]

T = {
    help_key.replace(OLD_HELP, OLD_HELP + ADDED): [
        text.replace(end, end + more) for text, end, more in zip(known[help_key], first_sentence_ends, help_added)
    ],
    "_Contact the author...": ["Contactea_ză autorul...", "Зв’язатися з _автором...", "_Contactar con el autor..."],
    "_Donate...": ["_Donează...", "По_жертвувати...", "_Donar..."],
    "_Yes": ["D_a", "_Так", "_Sí"],
    "_No": ["_Nu", "_Ні", "_No"],
    "Contact the author": ["Contactează autorul", "Зв’язатися з автором", "Contactar con el autor"],
    "Donate": ["Donează", "Пожертвувати", "Donar"],
    "Tin Pan is made by Ernesto Melendez. Enter opens the selected address. For a problem with Tin Pan, Help, Report a problem sends it straight to Ernesto with the details needed.": [
        "Tin Pan este făcut de Ernesto Melendez. Enter deschide adresa selectată. Pentru o problemă cu Tin Pan, Ajutor, Raportează o problemă o trimite direct lui Ernesto, cu detaliile necesare.",
        "Tin Pan створив Ернесто Мелендес. Enter відкриває вибрану адресу. Якщо з Tin Pan щось не так, «Довідка», «Повідомити про проблему» надсилає це просто Ернесто з потрібними подробицями.",
        "Tin Pan lo hace Ernesto Melendez. Intro abre la dirección seleccionada. Para un problema con Tin Pan, Ayuda, Informar de un problema se lo envía directamente a Ernesto con los detalles necesarios.",
    ],
    "Tin Pan is free. If it's useful to you, a donation helps keep it going. Enter opens the selected page in your browser.": [
        "Tin Pan este gratuit. Dacă îți este util, o donație ajută la continuarea lui. Enter deschide pagina selectată în browser.",
        "Tin Pan безкоштовний. Якщо він вам корисний, пожертва допомагає його розвивати. Enter відкриває вибрану сторінку в браузері.",
        "Tin Pan es gratis. Si te resulta útil, una donación ayuda a que siga adelante. Intro abre la página seleccionada en tu navegador.",
    ],
    "Email: {0}": ["Email: {0}", "Електронна пошта: {0}", "Correo: {0}"],
    "Mastodon: {0}": ["Mastodon: {0}", "Mastodon: {0}", "Mastodon: {0}"],
    "Telegram, news and updates for Ernesto's apps: {0}": [
        "Telegram, știri și actualizări pentru aplicațiile lui Ernesto: {0}",
        "Telegram, новини й оновлення застосунків Ернесто: {0}",
        "Telegram, noticias y actualizaciones de las aplicaciones de Ernesto: {0}",
    ],
    "GitHub, Tin Pan's code and releases: {0}": [
        "GitHub, codul și versiunile Tin Pan: {0}",
        "GitHub, код і випуски Tin Pan: {0}",
        "GitHub, el código y las versiones de Tin Pan: {0}",
    ],
    "PayPal: {0}": ["PayPal: {0}", "PayPal: {0}", "PayPal: {0}"],
    "Ko-fi: {0}": ["Ko-fi: {0}", "Ko-fi: {0}", "Ko-fi: {0}"],
    "Couldn't open {0}.": ["Nu s-a putut deschide {0}.", "Не вдалося відкрити {0}.", "No se pudo abrir {0}."],
}
