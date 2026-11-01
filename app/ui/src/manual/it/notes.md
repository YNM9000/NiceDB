# Note, script e snippet

## Aprire il pannello
Il pannello **Note, script e snippet** sta a destra della scheda. Si apre e si chiude con il pulsante col quaderno sulla riga delle schede o con [[key:panel_notes]]. Le schede del pannello sono **Note**, **Script** e **Snippet**; con le schede selezionate, [[←]] e [[→]] passano dall'una all'altra. Dalla palette: «Apri gli script», «Apri gli snippet».

Il pannello si allarga trascinandone il bordo sinistro. Mentre è aperto, la **Cronologia** laterale resta nascosta.

## Note
Una nota è un testo in Markdown legato a una connessione o a uno spazio di lavoro.

1. Apri **Note**.
2. Scegli a cosa si riferisce: **Connessione** (quella della scheda) o **Spazio** (lo spazio di lavoro attivo, oppure **Tutte le connessioni**).
3. Scrivi. Titoli con `#`, elenchi con `-`, codice fra tre apici inversi.

- La nota si salva da sola mentre scrivi; in alto si legge **Salvo…**, **Salvata** o, se qualcosa va storto, **Non salvata**.
- **Anteprima** mostra il Markdown impaginato, **Scrivi** torna al testo.
- **Cerca nelle note** cerca in tutte le note; un clic su un risultato apre quella nota.

## Script
Uno script è una query salvata con nome, cartella e tag.

### Salvare uno script
1. Nell'editor seleziona il testo da salvare, oppure lascia tutto senza selezione.
2. Premi **Salva script** nella barra della query, oppure [[key:save_script]].
3. Controlla **Nome** (parte dalla prima riga), **Cartella** e **Tag** separati da virgole.
4. In **Vale per** scegli la connessione della scheda o **Qualsiasi connessione**.
5. Premi **Salva**.

Una cartella si scrive come percorso, per esempio `Report/Incassi`. Se il testo ha dei parametri, lo script ne conserva anche i valori: vedi [Parametri nelle query](parameters).

### Usare gli script
L'elenco mostra gli script della connessione della scheda e quelli per qualsiasi connessione; **Tutte le connessioni** li mostra tutti. **Cerca negli script** guarda nome, cartella, tag e SQL.

| Azione | Come |
|---|---|
| Aprire in una nuova scheda | clic sullo script |
| Aprire ed eseguire | pulsante **Esegui** sulla riga |
| Inserire al cursore | pulsante **Inserisci nell'editor** (solo nella vista Query) |
| Rinominare | [[F2]] o **Azioni** › **Rinomina** |
| Spostare | **Azioni** › **Sposta in**, anche in una **Nuova cartella…** |
| Cambiare nome, cartella, tag e connessione | **Azioni** › **Modifica…** |
| Sostituire il testo con quello dell'editor | **Azioni** › **Aggiorna dall'editor** |
| Eliminare | [[Canc]] o **Azioni** › **Elimina**; [[Ctrl+Z]] lo riporta |

Uno script legato a un'altra connessione si apre su quella connessione. Gli script si trovano anche con **Cerca** nella barra del titolo.

## Snippet
Uno snippet è un pezzo di SQL con dei segnaposto, da inserire con una parola.

### Inserire uno snippet
- Nell'editor scrivi la parola dello snippet (per esempio `topn`, `countby`, `dups`, `join`, `cte`): compare nel completamento con il segno **§**; [[Invio]] la sostituisce con il testo.
- Oppure, in **Snippet**, fai clic sullo snippet: entra al cursore.

Il cursore va sul primo segnaposto evidenziato: [[Tab]] passa al successivo, [[Shift+Tab]] torna indietro, [[Esc]] esce. Un segnaposto ripetuto si scrive una volta sola.

### Creare uno snippet
1. In **Snippet** premi **Nuovo snippet**.
2. Compila **Nome**, **Parola da digitare** (lettere, cifre e `_`), **Motore** (**Tutti** o uno solo), **Descrizione** e **Testo**.
3. Nel testo scrivi i segnaposto come `${1:tabella}`, `${2:colonna}`; lo stesso numero in due punti si riempie insieme.
4. Premi **Salva**.

L'elenco divide **I miei snippet** da quelli **Di serie** per il motore della scheda. Uno snippet di serie non si modifica: tasto destro › **Copia tra i miei**, poi lo si cambia. Il tasto destro offre anche **Apri in una nuova scheda**, **Copia**, **Modifica…** ed **Elimina**.

```sql
select ${1:colonna}, count(*) as n
from ${2:tabella}
group by ${1:colonna}
order by n desc
```

## Dove stanno
Note, script e snippet stanno nel database locale dell'app. Viaggiano con il pacchetto `.nicedb` se all'esportazione scegli «Gli script, gli snippet e le note»: vedi [Spazi di lavoro e cartelle](workspaces).
