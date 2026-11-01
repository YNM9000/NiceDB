# Redis e Valkey

Redis e Valkey usano lo stesso driver. NiceDB riconosce Valkey dalle informazioni del server e lo chiama col suo nome nella barra di stato.

## Connettersi
1. **Nuova connessione**, poi in **Motore** scegli **Redis / Valkey**.
2. Compila **Host** e **Porta** (6379 di solito).
3. **Database (numero)**: il database logico da aprire, da 0 in su.
4. **Utente**: un utente ACL; vuoto vuol dire l'utente `default`, come per i server con la sola `requirepass`.
5. **Password**: vuota per un server senza password. Sta nel portachiavi.
6. **TLS**, **Tunnel SSH** e proxy come per gli altri motori. **Prova**, poi **Salva**.

Il **Modo** sopra l'host sceglie **Un server**, **Cluster** o **Sentinel**. Con Sentinel si indicano uno o più Sentinel (`s1:26379, s2:26379`) e il **Nome del master**: NiceDB si collega al master che indicano. Nell'esploratore, sotto la connessione, una riga dice quale master è in uso. Se i Sentinel lo spostano (failover), il vecchio master diventa una replica: NiceDB se ne accorge al primo comando rifiutato o al keep-alive, si ricollega al nuovo, lo dice con un avviso e la riga mostra l'ora del failover e il master di prima. **Strumenti › Server** ha la sezione **Sentinel** con il master, le repliche e quanti Sentinel lo seguono.

Le variabili `${NOME}` valgono in tutti i campi, anche nel numero del database (`${DB:-0}`). Ogni connessione si presenta al server come `NiceDB` in `CLIENT LIST`.

## Sfogliare le chiavi
Sotto la connessione ci sono i database (`db0`, `db1`…) e, nel database aperto, i **prefissi**: la parte del nome prima dei due punti, con quante chiavi ha ciascuno. Il database si cambia dalla tendina sopra l'esploratore.

Un clic su un prefisso apre il browser delle chiavi filtrato su `prefisso:*`. Il browser è la vista **Dati** delle connessioni Redis: a sinistra le chiavi, a destra il valore.

1. In **Filtra le chiavi** scrivi un pattern Redis (`cache:utente:*`, `sessione:web:0?d*`) e scegli il tipo nella tendina.
2. [[Invio]] rilegge da capo.
3. Le chiavi si raggruppano sui due punti in cartelle; accanto a ogni chiave ci sono tipo, scadenza e memoria occupata.
4. NiceDB legge 1000 chiavi alla volta con `SCAN`, mai con `KEYS`. In fondo c'è quante chiavi sono state lette su quante ne ha il database: **Carica altre** legge la pagina dopo, **Carica tutte** continua fino in fondo (al massimo 200 000), **Ferma** interrompe.

| Tasto | Effetto |
|---|---|
| [[↑]] [[↓]] | chiave precedente o successiva |
| [[→]] [[←]] | apre o chiude una cartella |
| [[Invio]] | apre la chiave |
| [[Canc]] | elimina la chiave |

Tasto destro su una cartella: **Mostra solo questa cartella**, **Copia il prefisso**. Su una chiave: **Copia il nome**, **Leggi nell'editor** (apre una scheda con `HGETALL`, `LRANGE`, `XRANGE`… pronto da eseguire), **Elimina la chiave**.

## Leggere e modificare un valore
In testa al valore ci sono nome, tipo, lunghezza, memoria e scadenza, con i pulsanti per copiare il nome, **Rileggi il valore**, **Rinomina**, **Cambia la scadenza** ed **Elimina la chiave**.

| Tipo | Come si vede | Cosa si modifica |
|---|---|---|
| Stringa | testo; se è JSON anche come albero | il testo, con **Salva** o [[Ctrl+Invio]]; la scadenza resta |
| Hash | campo e valore | il valore (doppio clic, [[Invio]] salva), nuovi campi, togliere un campo |
| Lista | indice e valore | il valore, aggiungere **In cima** o **In fondo**, togliere un elemento |
| Insieme | i membri | aggiungere e togliere membri |
| Insieme ordinato | membro e punteggio, dal più alto | il punteggio, nuovi membri, togliere un membro |
| Stream | ID e campi | nuove voci (un campo per riga: `stato spedito`), togliere una voce |
| JSON | albero o testo | il documento intero, se il modulo RedisJSON c'è |

- Le collezioni grandi arrivano a pagine di 200; in fondo alla tabella «carica altri».
- Le stringhe oltre 1 MB mostrano solo il primo MB e non si modificano; i valori binari si leggono soltanto, con le sequenze `\xNN`.
- Togliere un elemento da una lista controlla prima che a quell'indice ci sia ancora il valore letto.
- **Rinomina** non sovrascrive una chiave che esiste già.
- **Scadenza**: scrivi i secondi o un'unità (`90`, `15m`, `2h`, `7d`) e premi **Imposta**; **Senza scadenza** la toglie.
- **Elimina la chiave**, fuori dalla produzione, chiede un secondo clic: «Premi di nuovo per eliminare».

## Creare una chiave
1. **Nuova chiave** (il `+` in alto) o lo stesso comando dalla palette.
2. Scrivi **Nome della chiave**, scegli il **Tipo**, il primo valore e la **Scadenza** (vuota: non scade).
3. **Crea**. Se la chiave esiste già, NiceDB si ferma.

## Scrivere comandi
L'editor accetta comandi Redis, uno per riga, come in `redis-cli`: argomenti tra virgolette doppie con `\n` e `\xNN`, tra apici singoli alla lettera. Le righe che iniziano con `#` o `//` sono commenti.

```
# chi ha comprato di più
ZREVRANGE classifica:clienti:fatturato 0 9 WITHSCORES
HGETALL sessione:app:tablet-01
SET cache:listino:nota "aggiornato alle 9" EX 600
```

- [[key:run_statement]] esegue la riga sotto il cursore, [[key:run_script]] tutte in ordine; [[key:format]] mette in maiuscolo i nomi dei comandi.
- L'autocompletamento propone comandi, sottocomandi (`CONFIG GET`, `CLIENT LIST`…) e i nomi delle chiavi che iniziano con quello che scrivi.
- Ogni risposta diventa una griglia: un valore, un elenco con numero e valore, una mappa con campo e valore, le coppie (`WITHSCORES`) in colonne.
- [[key:cancel]] ferma anche i comandi che aspettano, come `BLPOP … 0`.
- Non partono dall'editor `SELECT` (usa la tendina dei database), `SUBSCRIBE`, `MONITOR`, `MULTI`/`EXEC`, `WATCH`, `HELLO`, `RESET`, `QUIT`.

## Produzione e registro
I comandi di lettura (`GET`, `HGETALL`, `SCAN`, `INFO`, `CONFIG GET`…) passano anche a scrittura bloccata; tutti gli altri chiedono lo sblocco. `FLUSHDB`, `FLUSHALL`, `KEYS`, `CONFIG SET`, `SHUTDOWN`, `DEBUG`, `CLIENT KILL`, `SWAPDB`, `REPLICAOF` chiedono conferma come rischiosi. Le modifiche dal browser mostrano nella conferma i comandi che partiranno e finiscono nel registro come li scriverebbe `redis-cli`. Vedi [Sicurezza in produzione](production).

## Il pannello del server
**Strumenti › Server** mostra versione, da quanto è acceso, comandi al secondo, client, memoria, chiavi e letture a segno. Sotto, quattro parti:
- **Informazioni**: le sezioni di `INFO` e i comandi più chiamati; **Filtra** cerca nomi e valori.
- **Client**: `CLIENT LIST`, con la riga di NiceDB evidenziata.
- **Comandi lenti**: `SLOWLOG GET 128`.
- **Memoria**: `MEMORY STATS` e il parere di `MEMORY DOCTOR`.

Se l'utente non ha il permesso per una parte, il pannello lo dice al suo posto.

Nella palette dei comandi, con una scheda Redis in primo piano: «Nuova chiave», «Rileggi le chiavi», «Carica tutte», «Apri Server».

## Limiti
- I conteggi delle cartelle valgono per le chiavi già lette finché la scansione non è completa.
- I prefissi dell'esploratore vengono da al massimo 20 000 chiavi: su database enormi un prefisso raro può mancare.
- Dopo un failover il comando che ha trovato la replica non viene ripetuto: va rilanciato.
- Le chiavi con byte non UTF-8 nel nome non si aprono dal browser; dall'editor sì, tra virgolette.
- Nessun certificato client per TLS reciproco. Attraverso tunnel o proxy il server si vede come `127.0.0.1`: con **Verifica il certificato** il certificato deve valere anche per quel nome.
- La memoria delle collezioni grandi è una stima.
- Moduli diversi da RedisJSON si usano solo dall'editor.
