# StockWatch

cargo install cargo-edit
`
    cargo add
    cargo rm
    cargo upgrade
`
async_once 支持await的lazy_static实现
 cargo add async_once 
 
 # docker 中下载 mysql
 docker pull mysql
 
 #启动
 docker run --name mysql -p 3306:3306 -e MYSQL_ROOT_PASSWORD=Lzslov123! -d mysql
 
 #进入容器
 docker exec -it mysql bash
 
 #登录mysql
 mysql -u root -p
 ALTER USER 'root'@'localhost' IDENTIFIED BY 'Lzslov123!';
 
 #添加远程登录用户
 CREATE USER 'liaozesong'@'%' IDENTIFIED WITH mysql_native_password BY 'Lzslov123!';
 GRANT ALL PRIVILEGES ON *.* TO 'liaozesong'@'%';
 flush privileges;
