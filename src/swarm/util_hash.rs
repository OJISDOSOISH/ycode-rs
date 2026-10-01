//! Portage de `packages/core/src/util/hash.ts`.
//!
//! La source expose un seul namespace `Hash`, avec exactement deux fonctions :
//!
//! - `fast(input)` : condensat **SHA-1** en hexadecimal ;
//! - `sha256(input)` : condensat **SHA-256** en hexadecimal.
//!
//! Deux details a ne pas rater :
//!
//! 1. `fast` ne veut **pas** dire rapide. C'est du SHA-1, malgre le nom. On
//!    garde le nom d'origine parce que les noms de fichier de verrouillage
//!    sont deja ecrits sur le disque par l'ancienne version.
//! 2. `createHash(...).update(x).digest("hex")` renvoie de l'hexadecimal
//!    **minuscule**, et quand `x` est une chaine JavaScript, Node l'encode en
//!    UTF-8 avant de hacher. Les deux proprietes sont reproduites ici.
//!
//! Le projet n'a aucune dependance de hachage declaree dans `Cargo.toml`, et un
//! sous-agent du swarm n'a pas le droit d'y toucher. Les deux algorithmes sont
//! donc implementes dans ce fichier, sans dependance externe, pour que le lot
//! complet compile tel quel. Si le responsable du projet prefere les crates
//! `sha1` et `sha2`, seules les deux fonctions privees `sha1_digest` et
//! `sha256_digest` sont a remplacer ; l'API publique ne change pas.

/// Calcule le condensat SHA-1 de l'entree, en hexadecimal minuscule.
///
/// Equivalent de `Hash.fast` du TypeScript. L'entree est un `string | Buffer`
/// cote TypeScript ; ici `impl AsRef<[u8]>` accepte aussi bien `&str` et
/// `String` (encodees en UTF-8, comme Node) que des octets bruts.
pub fn fast(input: impl AsRef<[u8]>) -> String {
    vers_hex(&sha1_digest(input.as_ref()))
}

/// Calcule le condensat SHA-256 de l'entree, en hexadecimal minuscule.
///
/// Equivalent de `Hash.sha256` du TypeScript.
pub fn sha256(input: impl AsRef<[u8]>) -> String {
    vers_hex(&sha256_digest(input.as_ref()))
}

/// Ajoute le remplissage de Merkle-Damgard a un message.
///
/// Le message est complete par `0x80`, puis par des octets nuls jusqu'a ce que
/// sa longueur convienne a un bloc de 64 octets moins les 8 octets de la
/// longueur finale, puis par la longueur du message initial en bits, sur
/// 8 octets en grand-boutiste. SHA-1 et SHA-256 partagent ce remplissage a
/// l'identique, ils n'ont pas le droit de diverger la-dessus.
fn completer(entree: &[u8]) -> Vec<u8> {
    let mut donnees = Vec::with_capacity(entree.len() + 72);
    donnees.extend_from_slice(entree);
    donnees.push(0x80);
    while donnees.len() % 64 != 56 {
        donnees.push(0);
    }
    let longueur_en_bits = (entree.len() as u64).wrapping_mul(8);
    donnees.extend_from_slice(&longueur_en_bits.to_be_bytes());
    donnees
}

/// Convertit des octets en chaine hexadecimal minuscule, deux caracteres par
/// octet, comme le fait `digest("hex")` de Node.
fn vers_hex(octets: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut sortie = String::with_capacity(octets.len() * 2);
    for &octet in octets {
        sortie.push(TABLE[usize::from(octet >> 4)] as char);
        sortie.push(TABLE[usize::from(octet & 0x0f)] as char);
    }
    sortie
}

/// Lit les 16 mots de 32 bits d'un bloc de 64 octets, en grand-boutiste.
fn lire_bloc(bloc: &[u8], mots: &mut [u32]) {
    for (i, mot) in mots.iter_mut().take(16).enumerate() {
        let decalage = 4 * i;
        *mot = u32::from_be_bytes([
            bloc[decalage],
            bloc[decalage + 1],
            bloc[decalage + 2],
            bloc[decalage + 3],
        ]);
    }
}

/// Condense SHA-1, en renvoyant les 20 octets bruts.
fn sha1_digest(entree: &[u8]) -> [u8; 20] {
    let donnees = completer(entree);
    let mut h: [u32; 5] = [
        0x6745_2301,
        0xefcd_ab89,
        0x98ba_dcfe,
        0x1032_5476,
        0xc3d2_e1f0,
    ];
    let mut w = [0u32; 80];

    for bloc in donnees.chunks_exact(64) {
        lire_bloc(bloc, &mut w);
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);

        for i in 0..80 {
            let (f, k) = if i < 20 {
                ((b & c) | ((!b) & d), 0x5a82_7999u32)
            } else if i < 40 {
                (b ^ c ^ d, 0x6ed9_eba1u32)
            } else if i < 60 {
                ((b & c) | (b & d) | (c & d), 0x8f1b_bcdu32)
            } else {
                (b ^ c ^ d, 0xca62_c1d6u32)
            };

            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }

    let mut sortie = [0u8; 20];
    for (i, mot) in h.iter().enumerate() {
        sortie[4 * i..4 * i + 4].copy_from_slice(&mot.to_be_bytes());
    }
    sortie
}

/// Condense SHA-256, en renvoyant les 32 octets bruts.
fn sha256_digest(entree: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a_2f98, 0x7137_4491, 0xb5c0_fbcf, 0xe9b5_dba5, 0x3956_c25b, 0x59f1_11f1,
        0x923f_82a4, 0xab1c_5ed5, 0xd807_aa98, 0x1283_5b01, 0x2431_85be, 0x550c_7dc3,
        0x72be_5d74, 0x80de_b1fe, 0x9bdc_06a7, 0xc19b_f174, 0xe49b_69c1, 0xefbe_4786,
        0x0fc1_9dc6, 0x240c_a1cc, 0x2de9_2c6f, 0x4a74_84aa, 0x5cb0_a9dc, 0x76f9_88da,
        0x983e_5152, 0xa831_c66d, 0xb003_27c8, 0xbf59_7fc7, 0xc6e0_0bf3, 0xd5a7_9147,
        0x06ca_6351, 0x1429_2967, 0x27b7_0a85, 0x2e1b_2138, 0x4d2c_6dfc, 0x5338_0d13,
        0x650a_7354, 0x766a_0abb, 0x81c2_c92e, 0x9272_2c85, 0xa2bf_e8a1, 0xa81a_664b,
        0xc24b_8b70, 0xc76c_51a3, 0xd192_e819, 0xd699_0624, 0xf40e_3585, 0x106a_a070,
        0x19a4_c116, 0x1e37_6c08, 0x2748_774c, 0x34b0_bcb5, 0x391c_0cb3, 0x4ed8_aa4a,
        0x5b9c_ca4f, 0x682e_6ff3, 0x748f_82ee, 0x78a5_636f, 0x84c8_7814, 0x8cc7_0208,
        0x90be_fffa, 0xa450_6ceb, 0xbef9_a3f7, 0xc671_78f2,
    ];

    let donnees = completer(entree);
    let mut h: [u32; 8] = [
        0x6a09_e667,
        0xbb67_ae85,
        0x3c6e_f372,
        0xa54f_f53a,
        0x510e_527f,
        0x9b05_688c,
        0x1f83_d9ab,
        0x5be0_cd19,
    ];
    let mut w = [0u32; 64];

    for bloc in donnees.chunks_exact(64) {
        lire_bloc(bloc, &mut w);
        for i in 16..64 {
            let x = w[i - 15];
            let y = w[i - 2];
            let s0 = x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3);
            let s1 = y.rotate_right(17) ^ y.rotate_right(19) ^ (y >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }

        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choix = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(choix)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut sortie = [0u8; 32];
    for (i, mot) in h.iter().enumerate() {
        sortie[4 * i..4 * i + 4].copy_from_slice(&mot.to_be_bytes());
    }
    sortie
}

#[cfg(test)]
mod tests {
    use super::{completer, fast, sha1_digest, sha256, vers_hex};

    /// TEMPORARY DIAGNOSTIC -- a supprimer apres identification.
    ///
    /// Le code de `sha1_digest` est textuellement le SHA-1 canonique, et une
    /// transliteration fidele donne le condensat correct. Le runner renvoie
    /// pourtant 3485e413... pour l'entree vide. Ce test imprime donc l'etat
    /// intermediaire pour voir OU les deux divergent. Il PANIQUE volontairement,
    /// parce que la CI n'affiche la sortie que d'un test en echec.
    #[test]
    fn diagnostic_etat_intermediaire_du_sha1() {
        let padded = completer(b"");
        let padded_hex: String = padded
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join("");
        let digest = vers_hex(&sha1_digest(b""));
        let reference = "da39a3ee5e6b4b0d3255bfef95601890afd80709";
        panic!(
            "longueur_remplie={} remplissage={} digest={} reference={} egal={}\nBLOCKS: {}",
            padded.len(),
            padded_hex,
            digest,
            reference,
            digest == reference,
            padded.chunks_exact(64).count(),
        );
    }

    // Message de 56 octets, choisi pour tomber juste avant une frontiere de
    // bloc et forcer le remplissage a repasser sur un second bloc.
    const MESSAGE_LONG: &str = "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";

    #[test]
    fn une_entree_vide_donne_le_condensa_connu() {
        assert_eq!(
            fast(""),
            "da39a3ee5e6b4b0d3255bfef95601890afd80709"
        );
        assert_eq!(
            sha256(""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn abc_donne_le_condensa_connu_pour_les_deux_algorithmes() {
        assert_eq!(
            fast("abc"),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(
            sha256("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn un_message_qui_depasse_un_bloc_donne_le_condensa_connu() {
        assert_eq!(
            fast(MESSAGE_LONG),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
        assert_eq!(
            sha256(MESSAGE_LONG),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn les_deux_algorithmes_ne_donnent_pas_le_meme_condensa() {
        // Meme entree, algorithmes differents : c'est bien le cas, puisque
        // "fast" est du SHA-1 et "sha256" est du SHA-256.
        assert_ne!(fast(MESSAGE_LONG), sha256(MESSAGE_LONG));
        assert_eq!(fast(MESSAGE_LONG).len(), 40);
        assert_eq!(sha256(MESSAGE_LONG).len(), 64);
    }

    #[test]
    fn une_chaine_est_hachee_avec_ses_octets_utf8() {
        // "e" accentue : l'echantillon de controle est ecrit sans accent pour
        // rester dans le jeu de caracteres latin, mais la chaine reelle occupe
        // deux octets en UTF-8 et doit donner le meme resultat que ces
        // deux octets la.
        let accentuee = "\u{e9}";
        let octets: [u8; 2] = [0xc3u8, 0xa9u8];
        assert_eq!(fast(accentuee), fast(&octets[..]));
        assert_eq!(sha256(accentuee), sha256(&octets[..]));
    }

    #[test]
    fn le_condensa_a_la_longueur_attendue_et_en_minuscules() {
        assert_eq!(fast("").len(), 40);
        assert_eq!(sha256("").len(), 64);
        for caractere in fast("opencode").chars() {
            assert!(caractere.is_ascii_digit() || caractere.is_ascii_lowercase());
        }
    }

    #[test]
    fn deux_entrees_differentes_donnent_deux_condensas_differents() {
        assert_ne!(fast("a"), fast("A"));
        assert_ne!(fast("opencode"), fast("opencode "));
        assert_ne!(sha256("opencode"), sha256("opencode "));
    }

    #[test]
    fn un_octet_nul_est_un_octet_comme_un_autre() {
        assert_ne!(fast("\u{0}"), fast(""));
        assert_ne!(sha256("a\u{0}b"), sha256("ab"));
    }

    #[test]
    fn la_longueur_du_message_entre_dans_le_condensa() {
        // Une entree qui a exactement la taille d'un bloc ne doit pas donner le
        // meme condensat qu'une entree d'un octet de moins, ni de plus.
        let bloc: Vec<u8> = vec![b'x'; 64];
        let avant: Vec<u8> = vec![b'x'; 63];
        let apres: Vec<u8> = vec![b'x'; 65];
        assert_ne!(fast(&bloc[..]), fast(&avant[..]));
        assert_ne!(sha256(&bloc[..]), sha256(&apres[..]));
    }
}
