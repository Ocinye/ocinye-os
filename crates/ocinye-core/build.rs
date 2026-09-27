// As migrações entram no binário pelo `sqlx::migrate!` (`src/db.rs`), no
// momento da compilação. O cargo só recompila este crate quando um ficheiro que
// conhece muda — e uma migração **nova** é um ficheiro que ele não conhece.
// Sem esta linha, um release cujas mudanças fossem só migrações novas sairia,
// com a cache de compilação dos deploys, com um binário que não as tem: o
// esquema ficava para trás e ninguém o via. Foi o que a prova de actualização
// da Parte 10 apanhou.
fn main() {
    println!("cargo:rerun-if-changed=../../migrations");
}
