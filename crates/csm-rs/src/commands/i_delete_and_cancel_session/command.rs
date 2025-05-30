use std::time::Instant;

use crate::{
  bss::{self, types::BootParameters},
  cfs::{
    self, component::http_client::v2::types::Component,
    session::utils::get_list_xnames_related_to_session,
  },
  error::Error,
  ims,
};
use dialoguer::{theme::ColorfulTheme, Confirm};

pub async fn exec(
  shasta_token: &str,
  shasta_base_url: &str,
  shasta_root_cert: &[u8],
  hsm_group_available_vec: Vec<String>,
  cfs_session_name: &str,
  dry_run: bool,
  assume_yes: bool,
) -> Result<(), Error> {
  log::info!("Deleting session '{}'", cfs_session_name);

  // Get collectives (CFS configuration, CFS session, BOS session template, IMS image and CFS component)
  let start = Instant::now();
  log::info!("Fetching data from the backend...");
  let (mut cfs_session_vec, cfs_component_vec, bss_bootparameters_vec) = tokio::try_join!(
    cfs::session::http_client::v2::get_all(
      shasta_token,
      shasta_base_url,
      shasta_root_cert,
    ),
    cfs::component::http_client::v2::get_all(
      shasta_token,
      shasta_base_url,
      shasta_root_cert,
    ),
    bss::http_client::get_all(shasta_token, shasta_base_url, shasta_root_cert)
  )?;
  let duration = start.elapsed();
  log::info!(
    "Time elapsed to fetch information from backend: {:?}",
    duration
  );

  // Validate:
  // - Check CFS session belongs to a cluster available to the user
  // - Check CFS session to delete exists
  // - CFS configuration related to CFS session is not being used to create an image
  // - CFS configuration related to CFS session is not a desired configuration
  //
  // Get CFS session to delete
  // Filter CFS sessions based on use input
  // let mut cfs_session_vec = cfs_session_vec_opt.unwrap_or_default();

  // Check CFS session belongs to a cluster the user has access to (filter sessions by HSM
  // group)
  cfs::session::utils::filter_by_hsm(
    shasta_token,
    shasta_base_url,
    shasta_root_cert,
    &mut cfs_session_vec,
    &hsm_group_available_vec,
    None,
    false,
  )
  .await?;

  // Check CFS session to delete exists (filter sessions by name)
  let cfs_session = cfs_session_vec
    .iter()
    .find(|cfs_session| {
      cfs_session.name.eq(&Some(cfs_session_name.to_string()))
    })
    .ok_or_else(|| {
      Error::Message(format!(
        "CFS session '{}' not found. Exit",
        cfs_session_name
      ))
    })?;

  // Get xnames related to CFS session to delete:
  // - xnames belonging to HSM group related to CFS session
  // - xnames in CFS session
  let xname_vec = get_list_xnames_related_to_session(
    shasta_token,
    shasta_base_url,
    shasta_root_cert,
    cfs_session.clone(),
  )
  .await?;

  let cfs_session_target_definition = cfs_session.get_target_def().unwrap();

  // DELETE DATA
  //
  // * if session is of type dynamic (runtime session) then:
  // Get retry_policy
  if cfs_session_target_definition == "dynamic" {
    // The CFS session is of type 'target dynamic' (runtime CFS batcher) - cancel session by
    // setting error_count to retry_policy value
    log::info!("CFS session target definition is 'dynamic'.");

    let cfs_global_options = cfs::component::http_client::v3::get_options(
      shasta_token,
      shasta_base_url,
      shasta_root_cert,
    )
    .await?;

    let retry_policy = cfs_global_options["default_batcher_retry_policy"]
      .as_u64()
      .unwrap();

    if !assume_yes {
      // Ask user for confirmation
      let user_msg = format!(
        "Session '{}' will get canceled:\nDo you want to continue?",
        cfs_session_name,
      );
      if Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(user_msg)
        .interact()
        .unwrap()
      {
        log::info!("Continue",);
      } else {
        println!("Cancelled by user. Aborting.");
        return Ok(());
      }
    }

    cancel_session(
      shasta_token,
      shasta_base_url,
      shasta_root_cert,
      xname_vec,
      Some(cfs_component_vec),
      retry_policy,
      dry_run,
    )
    .await?;
  } else if cfs_session_target_definition == "image" {
    // The CFS session is not of type 'target dynamic' (runtime CFS batcher)
    let image_created_by_cfs_session_vec = cfs_session.get_result_id_vec();
    if !image_created_by_cfs_session_vec.is_empty() {
      if !assume_yes {
        // Ask user for confirmation
        let user_msg = format!(
                    "Images listed below which will get deleted:\n{}\nDo you want to continue?",
                    image_created_by_cfs_session_vec.join("\n"),
                );
        if Confirm::with_theme(&ColorfulTheme::default())
          .with_prompt(user_msg)
          .interact()
          .unwrap()
        {
          log::info!("Continue",);
        } else {
          println!("Cancelled by user. Aborting.");
          return Ok(());
        }
      }

      // Delete images
      delete_images(
        shasta_token,
        shasta_base_url,
        shasta_root_cert,
        &image_created_by_cfs_session_vec,
        &bss_bootparameters_vec,
        dry_run,
      )
      .await?;
    }
  } else {
    return Err(Error::Message(format!(
      "CFS session target definition is '{}'. Don't know how to continue. Exit",
      cfs_session_target_definition
    )));
  };

  // Delete CFS session
  if dry_run {
    println!("Dry Run Mode: Delete CFS session '{}'", cfs_session_name);
  } else {
    cfs::session::http_client::v3::delete(
      shasta_token,
      shasta_base_url,
      shasta_root_cert,
      &cfs_session_name,
    )
    .await?;
  }

  Ok(())
}

async fn delete_images(
  shasta_token: &str,
  shasta_base_url: &str,
  shasta_root_cert: &[u8],
  image_created_by_cfs_session_vec: &[String],
  bss_bootparameters_vec_opt: &[BootParameters],
  dry_run: bool,
) -> Result<(), Error> {
  // Delete images
  for image_id in image_created_by_cfs_session_vec {
    let is_image_boot_node = bss_bootparameters_vec_opt
      .iter()
      .any(|boot_parameters| boot_parameters.get_boot_image().eq(image_id));

    if !is_image_boot_node {
      if dry_run {
        println!(
                    "Dry Run Mode: CFS session target definition is 'image'. Deleting image '{}'",
                    image_id
                );
      } else {
        ims::image::http_client::delete(
          shasta_token,
          shasta_base_url,
          shasta_root_cert,
          image_id,
        )
        .await?;
      }
    } else {
      println!(
        "Image '{}' is a boot node image. It will not be deleted.",
        image_id
      );
    }
  }

  Ok(())
}

async fn cancel_session(
  shasta_token: &str,
  shasta_base_url: &str,
  shasta_root_cert: &[u8],
  xname_vec: Vec<String>,
  cfs_component_vec_opt: Option<Vec<Component>>,
  retry_policy: u64,
  dry_run: bool,
) -> Result<(), Error> {
  // Set CFS components error_count == retry_policy so CFS batcher stops retrying running
  log::info!(
    "Set 'error_count' {} to xnames {:?}",
    retry_policy,
    xname_vec
  );

  // Update CFS component error_count
  let cfs_component_vec: Vec<Component> = cfs_component_vec_opt
    .expect("No CFS components")
    .iter()
    .filter(|cfs_component| {
      xname_vec.contains(
        &cfs_component
          .id
          .as_ref()
          .expect("CFS component found but it has no id???"),
      )
    })
    .cloned()
    .collect();

  log::info!(
    "Update error count on nodes {:?} to {}",
    xname_vec,
    retry_policy
  );

  if dry_run {
    println!(
      "Dry Run Mode: Update error count on nodes {:?}",
      cfs_component_vec
    );
  } else {
    cfs::component::http_client::v2::put_component_list(
      shasta_token,
      shasta_base_url,
      shasta_root_cert,
      cfs_component_vec,
    )
    .await?;
  }

  Ok(())
}
