# ODBC

Con il motore ODBC NiceDB si collega a qualsiasi database che abbia un driver ODBC: Informix, Teradata, Sybase, Progress, il driver di un gestionale.

## Connettersi
1. Installa il driver ODBC del database (dal produttore o dal gestore di pacchetti).
2. **Nuova connessione**, poi in **Motore** scegli ODBC.
3. Scegli come trovare il driver:
   - **DSN**: il nome di un'origine dati già configurata in `odbc.ini` o nel pannello «Origine dati ODBC» di Windows;
   - **Driver**: il percorso della libreria del driver (`/usr/lib64/psqlodbcw.so`) o il suo nome registrato.
4. **Stringa di connessione**: le parole chiave del driver, per esempio `Server=db.lan;Port=5432;Database=gestionale`.
5. **Utente** e **password**; la password sta nel portachiavi e va al driver come `PWD`.

## Cosa si può fare
Query e script, risultati, vista Dati con le modifiche, transazioni (`BEGIN TRANSACTION`, `COMMIT`, `ROLLBACK`), Ferma, l'albero con schemi, tabelle e viste, «Mostra DDL» delle tabelle, confronto schemi e righe, import ed export, ricerca nei valori e dati di prova.

## Limiti
NiceDB non conosce il database che sta dietro il driver: niente Spiega, Attività, Monitoraggio, Spazio e Utenti. I nomi con maiuscole e minuscole mescolate o con caratteri speciali vanno tra virgolette doppie; con un driver che usa altre virgolette conviene tenere nomi semplici.
