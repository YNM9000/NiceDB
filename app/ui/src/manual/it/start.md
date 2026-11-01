# Primi passi

## Configurazione iniziale
Al primo avvio, dopo la schermata d'apertura, compare la **Configurazione iniziale**. Ogni scelta si vede subito nell'**Anteprima**.

| Passo | Cosa si sceglie |
|---|---|
| **Lingua** | lingua di menu, messaggi e avvisi |
| **Aspetto** | stile, palette e tema chiaro o scuro |
| **Comodità** | dimensione dell'interfaccia, carattere dell'editor, suoni e volume |
| **Sicurezza in produzione** | per quanto si sblocca la scrittura e come si conferma una scrittura |
| **Pronto** | riepilogo delle scelte |

- [[Invio]] passa al passo successivo, **Indietro** torna a quello prima.
- [[Esc]] o **Salta** chiudono la configurazione dopo una conferma: quello che non hai scelto resta com'è.
- Per rivederla: **Impostazioni › Aspetto › Rivedi la configurazione iniziale**.

Ogni scelta si cambia poi in [Impostazioni](settings). Le opzioni di sicurezza sono spiegate in [Sicurezza in produzione](production).

## Aggiungere la prima connessione
1. Premi **Nuova connessione** al centro della finestra, oppure il **+** accanto a **Connessioni** nella colonna di sinistra.
2. Scegli il motore nella lista a sinistra: scrivi le prime lettere per trovarlo, le frecce lo cambiano, [[Enter]] passa ai campi.
3. A destra scrivi **Nome**, poi host, porta, database, utente e password; per SQLite e DuckDB basta il file.
4. Premi **Prova**: accanto al pulsante compaiono prodotto, versione e database del server.
5. Premi **Salva**. La connessione compare nella colonna **Connessioni**.

Tutti i campi, il tunnel SSH, TLS e le opzioni di sessione sono in [Connessioni e profili](connections).

## Scrivere ed eseguire una query
1. Fai clic sulla connessione nella colonna di sinistra: si apre una scheda con l'editor e, sotto, lo spazio dei risultati.
2. Scrivi una query.
3. Premi [[key:run_statement]] per eseguire l'istruzione sotto il cursore, oppure [[key:run_script]] per eseguire tutto il testo.
4. Le righe arrivano nella griglia sotto l'editor; errori e conteggi stanno nella scheda **Messaggi**.

Per fermare una query in corso premi [[key:cancel]] o **Annulla**. Dettagli in [Editor e completamento](editor) e in [Eseguire e leggere i risultati](results).

## La finestra
| Zona | Cosa contiene |
|---|---|
| Barra del titolo | a sinistra il marchio, lo spazio di lavoro e il nome della connessione, con il bollino **produzione** quando serve; a destra le viste, **Cerca**, **Aiuto** e l'ingranaggio delle **Impostazioni** |
| Colonna **Connessioni** | cartelle, connessioni e, sotto la connessione aperta, l'albero degli oggetti del database |
| Riga delle schede | una scheda per query; a destra il modo di commit, la transazione aperta e il pulsante **Note, script e snippet** |
| Barra della query | **Esegui**, **Esegui tutto**, **Spiega**, **Prova a secco**, **Formatta**, **Salva script**, **Cronologia** |
| Editor e risultati | divisi da una barra che si trascina; con la barra selezionata, [[↑]] e [[↓]] la spostano |
| Barra di stato | stato della sessione, somme delle celle selezionate, numero di connessioni |

Le colonne laterali si allargano trascinandone il bordo; le larghezze restano legate allo spazio di lavoro.

## Le viste
Il selettore nella barra del titolo cambia la vista della scheda attiva:

| Vista | A cosa serve |
|---|---|
| **Query** | editor e risultati |
| **Dati** | la tabella scelta nell'albero, da sfogliare e modificare: [Vista Dati e modifiche](data) |
| **Schema** | il diagramma delle tabelle: [Schema ER e piani di esecuzione](er-plans) |
| **Strumenti** | attività del server, spazio, utenti, confronti, backup: [Strumenti](tools) |

Il selettore segue la scheda attiva e non compare finché non c'è una scheda aperta. Da tastiera: [[Tab]] fino al selettore, poi le frecce.

## Cercare e dare comandi
**Cerca**, o [[key:palette]], trova tabelle, connessioni, script e comandi. [[key:palette_commands]] apre direttamente i comandi. Vedi [Palette dei comandi e scorciatoie](palette).

## Aprire l'aiuto
[[F1]] o il pulsante **Aiuto** (il punto interrogativo accanto all'ingranaggio) aprono questo manuale sulla pagina della schermata in cui ti trovi. Con delle schede aperte si apre accanto al lavoro, in una colonna a destra: puoi leggere e intanto scrivere la query. Si trova anche nella palette dei comandi: «Apri il manuale».

| Tasto | Nel manuale |
|---|---|
| [[/]] | cerca in tutte le pagine; [[Invio]] apre il primo risultato, [[↓]] scende tra i risultati |
| [[Alt+←]] [[Alt+→]] | pagina precedente o successiva |
| [[↑]] [[↓]] | nell'indice, passa da una pagina all'altra |
| [[Esc]] (con il fuoco nel manuale) o [[F1]] | chiude il manuale |

Le animazioni partono da sole; il pulsante in alto a destra le ferma. Con il movimento ridotto del sistema si vede la prima immagine e l'animazione parte solo dal pulsante.

## Annullare
[[Ctrl+Z]] annulla l'ultima azione: una scheda chiusa, una connessione eliminata, una cartella spostata, un ordinamento. Molte azioni mostrano un avviso con **Annulla**. [[Ctrl+Shift+Z]] o [[Ctrl+Y]] ripetono. Commit e Rollback non si annullano: vedi [Transazioni e prova a secco](transactions).
