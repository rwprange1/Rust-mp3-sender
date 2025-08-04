# MP3 File Sender

- This program could be used for other file types but
it is built so if you dont have a thumb drive or
a reliable way to transfer larger files you can
send the files individually using this program.

## Senders:
- To run this program be in the mp3-sender dir, you will need a folder
which contains your files or know the relative path to it. 
- If you want detailed progress updates run without the '--release' flag this will cause
the program to run slower.
- **usage: cargo run --release 1 <Dir> <IP> <Port>**
- Note this program will use 9 ports in range [port,port+9)

## Receivers:
- To run its pretty simple put your pc's wifi ip and a port in the following commad
- **usage: cargo run --release <n> <IP> <Port>**
- If you want detailed progress updates run without the '--release' flag this will cause
  the program to run slower.
