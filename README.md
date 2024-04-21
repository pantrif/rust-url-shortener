# rust-url-shortener
A URL Shortener with mysql support.  
Using Bijective conversion between natural numbers (IDs) and short strings

# Installation
## Using docker compose
```
docker-compose up --build
```
## Using an existing mysql

Edit .env file to add connection strings for mysql  
Run mysql_init/create_table.sql  
```
cargo run
```

# Usage

## Create short url
```
curl -X POST -H "Content-Type:application/json" -d '{"url": "http://www.google.com"}' http://localhost:8081/
```
Expected output  
```
{"url":"localhost:8081/3"}
```

## Redirect
```
curl -v localhost:8081/3
```
Output  
```
 Trying 127.0.0.1:8081...
* Connected to localhost (127.0.0.1) port 8081 (#0)
> GET /3 HTTP/1.1
> Host: localhost:8081
> User-Agent: curl/7.87.0
> Accept: */*
>
* Mark bundle as not supporting multiuse
< HTTP/1.1 302 Found
< location: http://www.google.com
< server: Rocket
< permissions-policy: interest-cohort=()
< x-frame-options: SAMEORIGIN
< x-content-type-options: nosniff
< content-length: 0
< date: Sun, 21 Apr 2024 10:00:27 GMT
```

# Licence 
This module is open-sourced software licensed under the [MIT license](http://opensource.org/licenses/MIT)
