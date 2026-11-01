# Eseguire e leggere i risultati

## Eseguire
| Comando | Cosa esegue |
|---|---|
| **Esegui**, [[key:run_statement]] | l'istruzione sotto il cursore; con una selezione, la selezione |
| **Esegui tutto**, [[key:run_script]] | tutto il testo; con una selezione, la selezione |
| tasto destro › **Esegui selezione** | solo il testo selezionato |
| **Spiega** | il piano dell'istruzione sotto il cursore: [Schema ER e piani di esecuzione](er-plans) |
| **Prova a secco** | la scrittura dentro una transazione da confermare: [Transazioni e prova a secco](transactions) |

Se l'istruzione ha dei segnaposto come `:id`, sopra i risultati compare la barra dei parametri: [Parametri nelle query](parameters).

Prima di un `UPDATE` o `DELETE` senza `WHERE`, un `DROP`, un `TRUNCATE` o un `ALTER`, NiceDB chiede conferma e dice quante righe verrebbero toccate: **Esegui comunque** procede, **Annulla** rinuncia. Sulle connessioni di produzione valgono le regole di [Sicurezza in produzione](production).

Dopo un `CREATE`, `ALTER` o `DROP` lo schema si ricarica da solo.

## Fermare una query
Premi **Annulla** nella barra della query o [[key:cancel]]. La query viene annullata sul server, non solo nascosta. Le righe già arrivate restano nella griglia.

Con ClickHouse, mentre la query gira compare l'avanzamento: righe lette, byte e percentuale.

## Schede dei risultati
| Scheda | Contenuto |
|---|---|
| **Risultati** | la griglia; con più istruzioni che restituiscono righe, **Risultati 1**, **Risultati 2**… |
| **Grafico** | le righe come grafico: [Pannello valore e grafici](value) |
| **Prima e dopo** | le righe toccate da una prova a secco |
| **Messaggi** | ogni istruzione con l'esito, le righe toccate e il tempo |
| **Piano** | il piano dopo **Spiega** |

Accanto alle schede: il numero di righe e il tempo, **Valore**, **Copia** ed **Esporta** ([Importare ed esportare](import-export)). Il tasto destro su **Risultati** offre **Confronta con…** ([Strumenti](tools)) ed **Esporta**.

**Messaggi** si apre da solo quando un'istruzione fallisce o non restituisce righe. Un errore mostra il messaggio del server, riga e colonna, dettagli e **Suggerimento** quando il server li dà; il punto dell'errore è segnato anche nell'editor.

## Muoversi e selezionare nella griglia
| Tasto o gesto | Effetto |
|---|---|
| frecce | cella vicina |
| [[Ctrl]] + freccia | fino al bordo della griglia |
| [[Pag↑]] / [[Pag↓]] | una pagina |
| [[Home]] / [[Fine]] | inizio e fine della riga; con [[Ctrl]], prima e ultima cella |
| [[Shift]] + frecce o clic | estende la selezione |
| [[Ctrl]] + clic | aggiunge o toglie celle |
| clic sul numero di riga | seleziona la riga; trascinando, più righe |
| [[Shift+Spazio]] / [[Ctrl+Spazio]] | seleziona la riga / la colonna della cella |
| [[Ctrl+A]] o clic sull'angolo | seleziona tutto |
| [[Esc]] | lascia solo la cella attiva |
| bordo destro dell'intestazione | trascinato allarga la colonna; doppio clic la adatta al contenuto |

[[Shift+Invio]] apre il pannello valore della cella: [Pannello valore e grafici](value). Il tasto destro su una cella offre copia, ordinamento, selezione e **Confronta con…**.

## Somme della selezione
Con almeno due celle selezionate, la barra di stato mostra **Conteggio**, **Somma**, **Media**, **Min**, **Max** e **Distinti**. Le somme sono esatte anche con molti decimali. Un clic su un valore lo copia.

## Copiare
- [[Ctrl+C]] copia la selezione come testo separato da tabulazioni.
- **Copia** sopra la griglia: **Come testo (TSV)**, **Come CSV**, **Come JSON**, **Come INSERT**.
- Tasto destro › **Copia come**: **Testo con intestazioni**, CSV, JSON, INSERT; poi **Copia colonna** e, sull'intestazione, **Copia nome**.

## Ordinare
Un clic sull'intestazione ordina la colonna: crescente, decrescente, poi senza ordine. [[Shift]] + clic aggiunge la colonna all'ordinamento. Le stesse voci sono nel tasto destro: **Ordina crescente**, **Ordina decrescente**, **Togli ordinamento**.

L'ordinamento avviene sul server: NiceDB rilancia l'istruzione con un `ORDER BY` e sostituisce le righe. [[Ctrl+Z]] torna all'ordine di prima. Non si può ordinare un risultato che non viene da una `SELECT`, che scrive o che viene da più istruzioni insieme: il motivo compare sopra la griglia.

## Risultati lunghi
- **Impostazioni › Risultati › Tetto di righe per query** ferma la lettura oltre un numero di righe: «Mostrate le prime N righe: restringi la query per vederle tutte.»
- Il **Limite di righe** della connessione taglia le `SELECT` senza `LIMIT`: compare «Limite della connessione: mostrate le prime N righe» con **Carica tutto**, che rilegge l'istruzione senza quel limite e tiene l'ordinamento. Vedi [Connessioni e profili](connections).

## Connessione caduta
Se la connessione cade durante una query, sopra i risultati compare un avviso. La query non viene rilanciata da sola:
- per una lettura, **Rilancia da qui** la riesegue senza rischi;
- per una scrittura l'avviso dice che potrebbe essere stata eseguita, e il pulsante è **Rilancia comunque**;
- se la connessione non torna, **Riconnetti** riprova.

## Cronologia
**Cronologia** nella barra della query, o [[key:panel_history]], apre l'elenco delle query eseguite su questa connessione, con data, tempo e righe. Le query fallite sono segnate.
- **Cerca nella cronologia** filtra per testo.
- Un clic inserisce la query in fondo all'editor.
- Tasto destro: **Inserisci nell'editor**, **Apri in una nuova scheda**, **Copia**.

## Limiti
- La griglia dei risultati è in sola lettura: per modificare le righe si usa la vista **Dati** ([Vista Dati e modifiche](data)).
- Ordinare rilancia la query: su una query lenta l'ordine arriva con lo stesso tempo.
- MySQL non può ordinare un risultato con due colonne dello stesso nome.
- Il limite della connessione ferma la lettura annullando la query: il server può aver già calcolato più righe.
