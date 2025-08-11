
use std::{net::{TcpStream, TcpListener}, env, thread, cmp};
use threadpool::ThreadPool;
use std::fs::{read_dir, OpenOptions};
use std::io::{ BufReader, Read, Write};
use std::net::Shutdown;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::sync::{Arc, Mutex};


struct ErrCnt{
    errors: u8
}

struct Files{
    files: Vec<String>,
}

fn main(){
    let mut args: Vec<String> = env::args().collect();
    args.remove(0);
    
    

    let run = match args[0].parse::<i32>(){
        Ok(i) =>{i}
        Err(_) =>{
            println!("Not a number! {}", args[0]);
            exit(1);
        }
    };
    
    let index;
    if run == 1{
        if args.len() != 4 {
            println!("Usage: cargo run --release <1:sender, n:receiver> <Music Dir> <receiver IP> <port>");
            println!("Server does not need to have a Music Dir");
            exit(1);
        }
        index = 2;
    }else {
        if args.len() != 3 {
            println!("Usage: cargo run --release <1:sender, n:receiver> <receiver IP> <port>");
            println!("Server does not need to have a Music Dir");
            exit(1);
        }
        index = 1;
    }
    
    
    
    let port = match args[index+1].parse::<u64>(){
        Ok(p) => {
            if p > 65535{
                println!("Not a valid port");
                exit(1);
            }
            p
        },
        Err(_)=> {
            println!("Not a valid port: {}", args[3]);
            exit(1);
        }
    };
    
    match run {
        0 => {
            let dir = Path::new(&args[1]);
            if !dir.exists() || !dir.is_dir(){
                println!("{} is not a directory", args[1]);
                exit(1);
            }
            send_mp3(&args[1], &args[2], port)
        },
        _=> {
            server(&args[1], port)
        }
    }
    
}

fn send_mp3(file_path: &String, ip: &String, port: u64) {
    let mut num_threads = 10;
    let pool = ThreadPool::new(num_threads);
    let dir = Path::new(file_path);
    let mut files = Files{files: Vec::new()};

    if dir.is_dir(){
        
        let read_dir = dir.read_dir().unwrap();
        
        for entry in read_dir{
            files.files.push(entry.unwrap().path().to_str().unwrap().to_string());
        }
        num_threads = cmp::min(num_threads,  (files.files.len()/100usize) +1);
    }

    let shared_data = Arc::new(Mutex::new(files));
  
    for i in 0..num_threads{
        let data_clone = shared_data.clone();
        let file_path = String::from(file_path);
        let ip_clone = ip.clone();
        pool.execute(move || {
            send_file(data_clone, file_path, &ip_clone,i + port as usize);
        });

    }
    
    pool.join();
}




fn server(ip: &String, port: u64) {
    let shared_data = Arc::new(Mutex::new(ErrCnt{errors: 0}));
    let listener = TcpListener::bind(format!("{}:{}", ip,port)).unwrap();
    let mut threads= Vec::new();
    
    
    for stream in listener.incoming(){
        match stream {
            Ok(stream) => {
                let cloned_data = shared_data.clone();
                
                let handle = thread::spawn(move || {
                    read_file(stream, cloned_data);
                });
                threads.push(handle);
            },
            Err(e) => {
                println!("failed {}" ,e);
            }
        }
    }
    
    for handle in threads {
        handle.join().unwrap();
    }
}


fn read_file(mut stream: TcpStream, shared_data: Arc<Mutex<ErrCnt>>) {
   
    loop{
        let file_size = match  recv_file_size(&mut stream){
            Ok(size) => size,
            Err(e) => {
                println!("{}", e);
                return;
            }
        };
        send_ok(&mut stream);
        
        let name = recv_name(&mut stream);
        send_ok(&mut stream);

        

        let mut file = match OpenOptions::new().truncate(true).create(true).write(true).open(&name) {
            Ok(file) => file,
            Err(_) => {
                let mut data = shared_data.lock().unwrap();
                let e_name = format!("Error{}.mp3", data.errors);
                data.errors += 1;
                OpenOptions::new().truncate(true).create(true).write(true).open(&e_name).unwrap()
            }
        };
        
        
        println!("Starting download of {}", name);
       
        
        let mut buffer = [0u8; 1500];
        stream.set_nonblocking(false).unwrap();
        let mut total_bytes: u64 = 0;
        
        while total_bytes < file_size {
            let bytes = stream.read(&mut buffer).unwrap();
            if bytes == 0 {break;}
            total_bytes += bytes as u64;
            file.write_all(&buffer[..bytes]).unwrap();
        }
        
        println!("Processed {}, {} bytes downloaded",name, total_bytes);
        send_ok(&mut stream);
    }
}


fn send_file(shared_data: Arc<Mutex<Files>>, dir: String, ip: &String, port: usize){

    match TcpStream::connect(format!("{}:{}", ip, port)) {
        Ok(mut connection) => {
            let mut data = shared_data.lock().unwrap();
            loop {

                let file = match data.files.pop() {
                    Some(s) => {
                        s.clone()
                    },
                    None => { 
                        connection.shutdown(Shutdown::Both).unwrap();
                        return;
                   }
               };
                drop(data);
                
                let to_send = OpenOptions::new().read(true).open(&file).unwrap();
                let file_size = to_send.metadata().unwrap().len();

                send_file_size(&mut connection, file_size);
                await_ok(&mut connection);

                let rep = format!("{}\\", dir);
                send_name(&mut connection, &file.replace(rep.as_str(), ""));
                await_ok(&mut connection);


                
                println!("Sending {}", file);
                let mut reader = BufReader::new(to_send);
                let mut write_buf = [0u8; 1500];
                let mut bytes_written = 0;

                while bytes_written < file_size as usize {
                    
                    let bytes = reader.read(&mut write_buf).unwrap();
                    connection.write_all(&write_buf[..bytes]).unwrap();
                    connection.flush().unwrap();
                    bytes_written += bytes;
                }
               
                
                println!("Finished sending {}", file);
              
                
                let p = if await_ok(&mut connection){
                    format!("Successfully finished send of {}", file).to_string()
                }else{
                    "Failed to finish send".to_string()
                };

                
                println!("{}", p);
                data = shared_data.lock().unwrap();
            }
        },
        Err(e) => {
            println!("Failed to connect to server: {}", e);
        },


    }

    println!("Returning");
}

fn send_ok(stream: &mut TcpStream){
    stream.set_nonblocking(false).unwrap();
    stream.write_all(b"Ok").unwrap();
    stream.flush().unwrap();
    debug_assert!({
        println!("Sent Ok");
        true
    });
}


fn await_ok(connection: &mut TcpStream) -> bool {
    let mut ok_buff = [0u8;2];
    connection.set_nonblocking(false).unwrap();
    connection.read_exact(&mut ok_buff).unwrap();
    let string = String::from_utf8_lossy(&ok_buff).to_string();
    debug_assert!({
        println!("Ok: {}", string);
        true
    });
    string == "Ok"
}

fn send_file_size(stream: &mut TcpStream, file_size: u64){
    let bytes = file_size.to_be_bytes();
    debug_assert!({
        println!("Bytes sent: {:?}", bytes);
        true
    });
    
    stream.set_nonblocking(false).unwrap();
    stream.write_all(&file_size.to_be_bytes()).unwrap();
    stream.flush().unwrap();
    debug_assert!({
        println!("File size: {}", file_size);
        true
    });
    
}

fn recv_file_size(stream: &mut TcpStream) -> Result<u64, String>{
    stream.set_nonblocking(false).unwrap();
    let mut size_buf = [0u8;8];
    match stream.read_exact(&mut size_buf){
        Ok(_) => {},
        Err(_) => {return Err("Client finished sending".to_string())}
    }

    debug_assert!({
        println!("Bytes recv {:?}", size_buf);
        true
    });
    
    Ok(u64::from_be_bytes(size_buf))
}


fn send_name(stream: &mut TcpStream, name: &String){
    stream.set_nonblocking(false).unwrap();
    stream.write_all(name.as_bytes()).unwrap();
    stream.flush().unwrap();

    debug_assert!({
        println!("Sending {}", name);
        true
    });
}

fn recv_name(stream: &mut TcpStream) -> String{
    let mut name_buf = [0u8;1024];
    let name_size = stream.read(&mut name_buf).unwrap();
    let string = String::from_utf8_lossy(&name_buf[0..name_size]).to_string();
    debug_assert!({
        println!("Received {}", string);
        true
    });
    string
}