# Impostazioni

## Aprire le impostazioni
- Clic sull'ingranaggio a destra nella barra del titolo, oppure il comando «Apri le impostazioni» nella palette.
- A sinistra ci sono le sezioni: **Aspetto**, **Lingua e suoni**, **Editor**, **Risultati**, **Sicurezza**, **Scorciatoie da tastiera**, **Aggiornamenti**, **Informazioni**. Con il focus sull'elenco, [[↑]] e [[↓]] cambiano sezione.
- Ogni scelta vale subito e resta salvata. **Ripristina**, in testa alle sezioni Editor, Risultati e Sicurezza, rimette i valori di serie.

## Aspetto
### Stile
Lo stile cambia materiali, caratteri e segni dell'interfaccia; le funzioni restano le stesse. Al cambio di stile riparte l'animazione d'apertura.

| Stile | Carattere |
|---|---|
| **Registro** | libro mastro: carta rigata, matita e inchiostro |
| **Tavola** | tavola tecnica: pellicola, cartiglio e revisioni |
| **Banco** | banco di manovra: smalto, spie e chiave |
| **Listino** | listino da sala operativa: cifre fitte, filetti e spie (di serie) |

### Palette
Ogni stile ha tre palette, ognuna in chiaro e in scuro:

| Stile | Palette |
|---|---|
| Registro | Lino e ferrogallico, Pergamena e bistro, Carta da zucchero e carbone |
| Tavola | China, Seppia, Vyco |
| Banco | Castano e isabella, Ardesia e nebbia, Verde vagone e ottone |
| Listino | Nero e ambra, Grafite e ciano (di serie), Blu notte e avorio |

Nelle schede dello stile e della palette le frecce scelgono la voce accanto.

### Tema e dimensione
- **Tema**: **Sistema** segue il sistema operativo, oppure **Chiaro** o **Scuro** (di serie).
- **Dimensione interfaccia**: 90%, 100%, 110%, 125%, 150%. Ingrandisce tutta la finestra; il carattere dell'editor si regola a parte.
- **Rivedi la configurazione iniziale** riapre la configurazione del primo avvio (vedi sotto).

## Lingua e suoni
- **Lingua**: Italiano o English. Menu, messaggi e avvisi cambiano subito.
- **Suoni**: **Tutti**, **Ridotti** o **Spenti**. **Ridotti** suona solo per errori, query lunghe finite, scritture e produzione; vale anche con «riduci animazioni» attivo nel sistema.
- **Timbro**: **Marimba** o **Campanelle**. Gli stessi suoni, in due voci: legno o metallo.
- **Volume**: da 0 a 100%. **Prova** fa sentire un segnale.
- **Suoni dei gesti**: un tocco quando cambi una scelta o lasci un elemento trascinato. Spenti all'inizio.
- **Suono d'avvio**: tre note quando la scritta NiceDB parte verso la barra del titolo. Tace con **Ridotti**.
- **Ascolta**: un clic su un tasto fa sentire quel suono.

## Editor
| Impostazione | Scelte | Di serie |
|---|---|---|
| **Dimensione del carattere** | 11, 12, 13, 14, 15, 16, 18 | 13 |
| **Larghezza del rientro** | 2, 3, 4, 8 | 2 |
| **Autocompletamento** | Attivi, Spenti | Attivi |
| **Parole chiave maiuscole in Formatta** | Attivi, Spenti | Attivi |
| **Nomi sconosciuti sottolineati** | Attivi, Spenti | Attivi |
| **Errore in uno script** | **Fermati** o **Continua** con l'istruzione dopo | Fermati |

Vedi [Editor e completamento](editor).

## Risultati
| Impostazione | Scelte | Di serie |
|---|---|---|
| **Tetto di righe per query** | 1.000, 10.000, 100.000, 1 M, 2 M | 2 M |
| **Valori NULL** | `NULL`, `—`, **Vuoto** | `NULL` |
| **Date** | **Come arrivano**, gg/mm/aaaa | Come arrivano |
| **Numeri** | **Come arrivano**, **Con separatore delle migliaia** | Come arrivano |

- Oltre il tetto la lettura si ferma e il risultato è segnato come troncato. Il limite di righe di una connessione, se c'è, vale prima del tetto.
- NULL, date e numeri cambiano solo come si vedono le celle: copia, esportazione e modifica usano il valore vero.

## Sicurezza
- **Limite di tempo** per ogni ambiente (locale, sviluppo, staging, produzione): una query che lo supera viene annullata sul server.
- **Sblocco della scrittura**: **A tempo**, **Fino alla disconnessione** o **Sempre aperta**.
- **Durata dello sblocco in produzione**: vale solo per lo sblocco a tempo.
- **Conferma prima di scrivere**: **Scrivere il nome**, **Finestra di conferma** o **Nessuna**.

Cosa fa ogni scelta: [Sicurezza in produzione](production).

## Cambiare una scorciatoia
1. **Impostazioni › Scorciatoie da tastiera**.
2. Clic sulla scorciatoia da cambiare: compare «Premi i tasti…».
3. Premi la combinazione nuova. [[Esc]] rinuncia.

- Serve almeno un tasto come Ctrl, Alt o Shift; fanno eccezione [[Esc]] e i tasti funzione.
- Se la combinazione è già usata, compare «… è già usata da …» e resta quella vecchia.
- **Predefinita** rimette quella di serie, **Togli** lascia l'azione senza scorciatoia (**Nessuna**), **Ripristina tutte** rimette tutte quelle di serie.
- Le righe con il lucchetto (**Fissa**) non si cambiano: Annulla, Ripeti, Cambia spazio di lavoro, Copia, Incolla, Taglia, Seleziona tutto, Cerca nell'editor.

Elenco completo: [Palette dei comandi e scorciatoie](palette).

## Aggiornamenti
- **Versione installata**: la versione di questa copia.
- **Controlla all'avvio**: **Sì** (di serie) o **No**. Al massimo una volta al giorno, in silenzio: senza rete non dice niente.
- **Controlla ora**: controlla subito. Risponde «NiceDB è aggiornato.» oppure mostra la versione nuova con le sue novità.
- **Fonte**: l'indirizzo da cui NiceDB legge le versioni pubblicate.

Quando esce una versione nuova compare un riquadro in basso a destra:
- **Aggiorna e riavvia** scarica la versione, controlla la firma, sostituisce NiceDB e lo riapre;
- **Novità** apre questa sezione con le note della versione;
- **Più tardi** chiude il riquadro fino al prossimo avvio.

Ogni aggiornamento è firmato e NiceDB lo installa solo se la firma corrisponde alla chiave inclusa nell'app. Se NiceDB è installato dove non si può scrivere (per esempio dal pacchetto .deb), il file verificato va nella cartella Scaricati e il riquadro dice dove: si installa da lì.

## Informazioni
- **Versione**, **Build** (data e commit) e **Sistema**: da citare quando segnali un problema.
- **Collegamenti**: codice sorgente, autore e testo della licenza, aperti nel browser.
- **Licenza di NiceDB**: GPL 3.0 o successiva; **Mostra il testo** apre il testo completo.
- **Licenze di terze parti**: librerie, caratteri e suoni inclusi nell'app, raggruppati per licenza. Il campo cerca per nome o licenza (per esempio `mit` o `plex`); un clic su una voce mostra il testo della sua licenza.
- Il coniglio in basso a destra apre la pagina GitHub dell'autore.

## Configurazione iniziale
Al primo avvio, dopo l'animazione d'apertura, NiceDB mostra la **Configurazione iniziale** in cinque passi:

1. **Lingua**: Italiano o English.
2. **Aspetto**: stile, palette e tema, con l'anteprima a destra.
3. **Comodità**: dimensione dell'interfaccia, suoni, volume e carattere dell'editor.
4. **Sicurezza in produzione**: sblocco e conferma. Le scelte prudenti sono già fatte: sblocco a tempo per 10 minuti e nome della connessione a ogni scrittura.
5. **Pronto**: il riepilogo. **Inizia** apre l'app, **Nuova connessione** apre subito il form della connessione.

- [[Invio]] va avanti, **Indietro** torna al passo prima; i passi già visti si riaprono dall'elenco a sinistra.
- **Salta** chiude la configurazione; [[Esc]] fa lo stesso, al primo avvio dopo una conferma. Quello che non hai scelto resta com'è.
- Ogni scelta vale subito e si cambia poi da queste stesse sezioni. Per rivederla: **Impostazioni › Aspetto › Rivedi la configurazione iniziale**.
