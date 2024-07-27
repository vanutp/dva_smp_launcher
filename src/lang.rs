use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone)]
pub enum Lang {
    English,
    Russian,
}

pub struct Localization {
    pub exiting: &'static str,
    pub error_during_auth: &'static str,
    pub error_during_user_info: &'static str,
    pub error_use_internet_for_first_connection: &'static str,
    pub checking_files: &'static str,
}

const RUSSIAN_LOC: Localization = Localization {
    exiting: "Выход...",
    error_during_auth: "Ошибка при аутентификации",
    error_during_user_info: "Ошибка при получении информации о пользователе",
    error_use_internet_for_first_connection: "Используйте интернет для первого подключения",
    checking_files: "Проверка файлов...",
};

const ENGLISH_LOC: Localization = Localization {
    exiting: "Exiting...",
    error_during_auth: "Error during authentication",
    error_during_user_info: "Error during user info retrieval",
    error_use_internet_for_first_connection: "Use internet for first connection",
    checking_files: "Checking files...",
};  

pub fn get_loc(lang: &Lang) -> Localization {
    match lang {
        Lang::English => ENGLISH_LOC,
        Lang::Russian => RUSSIAN_LOC,
    }
}
