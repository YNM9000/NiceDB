# Palette dei comandi e scorciatoie

## Aprire la palette
- [[key:palette]] apre **Cerca ovunque** sul modo **Oggetti**. Fa lo stesso il campo **Cerca** nella barra del titolo.
- [[key:palette_commands]] la apre sul modo **Comandi**. Anche `>` all'inizio della ricerca passa ai comandi.
- Nella barra laterale, scrivi nel filtro e premi [[Invio]] per continuare la ricerca nella palette.

Le due scorciatoie si cambiano in **Impostazioni › Scorciatoie da tastiera**.

## Cercare
La palette ha tre modi; [[Tab]] passa al successivo, [[Shift+Tab]] al precedente. Mentre scrivi, accanto a ogni modo compare quanti risultati ha.

| Modo | Cosa trova |
|---|---|
| **Oggetti** | tabelle, viste, colonne, funzioni, sequenze, tipi, trigger e indici delle connessioni aperte, gli altri database, le sessioni e gli utenti letti dall'esploratore |
| **Libreria** | script, query nei segnalibri, connessioni, spazi di lavoro, note e snippet |
| **Comandi** | ogni azione dell'app, con la sua scorciatoia |

- La ricerca è approssimata: `rord` trova `righe_ordine`. Contano di più il nome esatto, l'inizio del nome e l'inizio di una parola.
- Più parole vanno trovate tutte: una nel nome, le altre anche nel contesto (schema, database, connessione, tabella). `fatture numero` trova la colonna `numero` di `fatture`.
- Parole di aiuto: `tabella`, `vista`, `colonna`, `funzione`, `sessione`, `kill`, `utente`, `permessi`. `kill 4412` porta alla sessione 4412.
- Gli oggetti vengono dallo schema già caricato: mentre scrivi non parte nessuna query sul server.

## Agire su una riga
- [[Invio]] fa l'azione principale, [[Shift+Invio]] la seconda.
- [[→]] a fine testo, [[Ctrl+Invio]] o il tasto destro mostrano tutte le azioni della riga; [[←]] o [[Esc]] tornano ai risultati.
- [[Ctrl+C]], senza testo selezionato nella casella, copia il nome della riga (per script, query e snippet copia lo SQL).

| Riga | [[Invio]] | [[Shift+Invio]] |
|---|---|---|
| Tabella, vista | **Apri i dati** | **Nuova query SELECT** |
| Colonna | apre i dati della tabella | **Copia nome** |
| Funzione | **Mostra DDL** | **Copia firma** |
| Altro database | **Apri una scheda su questo database** | **Nuova query qui** |
| Sessione sul server | **Ferma la query** | copia la query |
| Script | **Apri lo script** | **Esegui** |
| Connessione | **Connetti** o **Vai alla connessione** | **Nuova scheda** |
| Spazio di lavoro | **Passa a questo spazio** | |
| Comando | **Esegui** | |

Tabelle e viste hanno anche, tra le altre azioni, **Mostra nello schema ER**, **Modifica tabella…**, **Copia SELECT**, **Copia CREATE**, **Esporta dati…**, **Importa dati…**, **Dati di prova…**, la manutenzione del motore e **Dipendenze…**. Fermare una query o chiudere una sessione passa dalla conferma della produzione.

## Fissati, segnalibri e recenti
- [[Ctrl+B]] fissa in cima la riga attiva, qualunque sia; di nuovo, la toglie.
- [[Ctrl+D]] mette nei segnalibri una tabella, una vista o una query.
- Il comando «Aggiungi la query ai segnalibri» salva il testo selezionato nell'editor, o tutto il testo.
- A palette vuota compaiono **Fissati**, **Segnalibri** e **Usati di recente** (le ultime otto righe scelte); mentre cerchi salgono nella classifica.
- Dopo aver chiuso la palette, [[Ctrl+Z]] annulla l'ultimo fissato o segnalibro aggiunto o tolto.

## Usare un comando in due passi
I comandi che finiscono con «…» chiedono qualcosa prima di partire.
1. Apri i comandi con [[key:palette_commands]].
2. Scrivi una parte del nome, per esempio `confronta`, e scegli «Confronta lo schema con…».
3. [[Invio]]: la casella si svuota e in testa compare il nome del comando; sotto, l'elenco delle scelte.
4. Cerca e scegli con le frecce, poi [[Invio]]. Quando il comando chiede un valore scritto, scrivilo e premi [[Invio]].

![Un comando in due passi nella palette](anim:palette-two-step)

[[Esc]], oppure [[←]] o [[Backspace]] a casella vuota, torna all'elenco dei comandi.

| Comando | Chiede |
|---|---|
| Confronta lo schema con… | una connessione, prima quelle dello stesso motore |
| Esporta *tabella* in… | il formato: CSV, TSV, JSON, Excel, Parquet |
| Apri la query in… | un'altra connessione |
| Connetti a…, Nuova scheda su…, Disconnetti… | una connessione |
| Modifica / Duplica / Elimina la connessione… | una connessione |
| Usa il database…, Apri la tabella…, Dati di prova per… | un database o una tabella |
| Manutenzione del database… | un'operazione del motore |
| Passa allo spazio di lavoro…, Nuovo spazio di lavoro… | uno spazio, o un nome da scrivere |
| Rinomina la scheda… | il nome nuovo |

## Tasti della palette
| Tasto | Effetto |
|---|---|
| [[↑]] [[↓]] | riga precedente o successiva, in cerchio |
| [[Pag↑]] [[Pag↓]] | dieci righe |
| [[Ctrl+↑]] [[Ctrl+↓]], [[Ctrl+Home]] [[Ctrl+Fine]] | prima o ultima riga |
| [[Invio]], [[Shift+Invio]] | prima o seconda azione |
| [[→]] o [[Ctrl+Invio]], [[←]] | apre e chiude l'elenco delle azioni |
| [[Tab]], [[Shift+Tab]] | modo successivo o precedente |
| [[Ctrl+B]], [[Ctrl+D]], [[Ctrl+C]] | fissa, segnalibro, copia |
| [[Esc]] | chiude, o torna dalle azioni |

## Tutte le scorciatoie
Le scorciatoie con il valore attuale si cambiano in **Impostazioni › Scorciatoie da tastiera**; le altre sono fisse.

### App
| Tasto | Effetto |
|---|---|
| [[key:palette]] | Cerca ovunque |
| [[key:palette_commands]] | Palette dei comandi |
| [[key:new_tab]] | Nuova scheda sulla connessione attiva |
| [[key:save_script]] | Salva come script |
| [[key:panel_notes]] | Pannello note e script |
| [[key:panel_history]] | Pannello cronologia |
| [[Ctrl+Z]] | Annulla (fuori dai campi di testo) |
| [[Ctrl+Shift+Z]], [[Ctrl+Y]] | Ripeti |
| [[Ctrl+Alt+1]] | tutte le connessioni |
| [[Ctrl+Alt+2]] … [[Ctrl+Alt+9]] | spazi di lavoro, in ordine |
| [[F1]] | Aiuto sulla schermata attuale |
| [[F2]] | rinomina la scheda, la cartella o lo script col focus |

### Editor
| Tasto | Effetto |
|---|---|
| [[key:run_statement]] | Esegui lo statement sotto il cursore |
| [[key:run_script]] | Esegui lo script |
| [[key:dry_run]] | Prova a secco |
| [[key:cancel]] | Annulla l'esecuzione |
| [[key:format]] | Formatta |
| [[key:rename_alias]] | Rinomina l'alias |
| [[Ctrl+Spazio]] | apre il completamento |
| [[Ctrl+F]] | Cerca nell'editor |
| [[Ctrl+D]] | seleziona la prossima occorrenza |
| [[Ctrl+Shift+L]] | seleziona tutte le occorrenze |
| [[Ctrl+Alt+↑]] [[Ctrl+Alt+↓]] | aggiunge un cursore sopra o sotto |
| [[Ctrl]] + clic, [[Alt]] + trascina | aggiunge un cursore, selezione rettangolare |

### Griglia dei risultati e vista Dati
| Tasto | Effetto |
|---|---|
| frecce, [[Pag↑]] [[Pag↓]] | sposta la cella |
| [[Ctrl+↑]] [[Ctrl+↓]] [[Ctrl+←]] [[Ctrl+→]] | fino al bordo |
| [[Home]] [[Fine]], [[Ctrl+Home]] [[Ctrl+Fine]] | inizio o fine riga, prima o ultima cella |
| [[Shift+Spazio]], [[Ctrl+Spazio]], [[Ctrl+A]] | seleziona riga, colonna, tutto |
| [[Ctrl+C]] | copia la selezione |
| [[Shift+Invio]], [[Alt+Invio]] | pannello valore, riga come scheda |
| [[Invio]], [[F2]] o scrivere | modifica la cella (vista Dati) |
| [[Canc]], [[Backspace]] | mette NULL, svuota e modifica |
| [[Ctrl+Canc]] | segna o toglie la riga da cancellare |
| [[Invio]] [[Shift+Invio]] [[Tab]] [[Shift+Tab]], [[Esc]] | in modifica: conferma e sposta, annulla |
| [[Ctrl+S]] | Applica le modifiche in sospeso |
| [[F4]], [[Shift+F4]] | segue la chiave esterna, in una nuova scheda |
| [[Alt+←]] [[Alt+→]] | indietro e avanti tra le tabelle seguite |

### Pannello valore
| Tasto | Effetto |
|---|---|
| [[F2]] | modifica |
| [[Ctrl+Invio]], [[Esc]] | salva, annulla |
| [[Ctrl+Shift+C]] | copia il percorso del nodo JSON |
| [[Esc]] | chiude il pannello |

### Esploratore, barra laterale e altri
| Tasto | Dove | Effetto |
|---|---|---|
| frecce, [[Home]] [[Fine]] | esploratore | sposta, apre e chiude |
| [[Invio]], [[Spazio]] | esploratore | apre, apre o chiude il nodo |
| lettere | esploratore | salta al nome che inizia così |
| [[Alt+↑]] [[Alt+↓]] | connessioni | sposta la connessione o la cartella |
| [[Canc]] | script | elimina lo script |
| [[Invio]], [[Esc]] | barra dei parametri | esegue, torna all'editor |
| [[↑]] [[↓]] | Impostazioni | sezione precedente o successiva |
| [[Invio]], [[Esc]] | configurazione iniziale | avanti, salta |

I tasti del browser delle chiavi sono in [Redis e Valkey](redis).

## Limiti
- Gli oggetti vengono solo dalle connessioni e dai database aperti.
- Sessioni e utenti si rileggono ogni 5 secondi mentre la finestra è aperta, ma solo per le connessioni dove l'esploratore li ha già letti una volta.
- Ogni azione dei menu contestuali ha il suo comando, con la stessa icona; quelle che chiedono una connessione, un formato o uno spazio lo chiedono al secondo passo.
- Fissati e segnalibri non viaggiano nell'export `.nicedb`.
- I comandi di una tabella valgono per quella mostrata nella vista Dati; per le altre passa da **Oggetti**.
